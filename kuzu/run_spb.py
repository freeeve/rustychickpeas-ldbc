#!/usr/bin/env python3
"""Kùzu reference side for the SPB queries — the head-to-head against
rustychickpeas's hand-coded SPB family.

Loads the same N-Triples sample as a property graph, builds a BM25 full-text
index, and runs the five SPB queries:

  Q1 about-entity, Q2 category rollup  -- plain Cypher (the aggregation shape)
  Q3 full-text                         -- Kùzu's `fts` index (BM25)  <- fts race
  Q4 geo radius / Q5 geo+fts           -- two modes:
       (a) Kùzu-native Cypher Haversine over all places (no spatial index)
       (b) hybrid: our geo k-d tree pre-filters places, Kùzu does the rest

Correctness is cross-checked against results/spb.rust.json (emitted by the
`spb` binary). Run `cargo run --bin spb` first.

Usage: .venv-kuzu/bin/python kuzu/run_spb.py [sample.nt] [runs]
"""
import csv
import json
import os
import re
import statistics
import sys
import tempfile
import time

import kuzu

SAMPLE = sys.argv[1] if len(sys.argv) > 1 else "samples/spb-sample.nt"
RUNS = int(sys.argv[2]) if len(sys.argv) > 2 else 5
QLAT, QLON, RADIUS = 51.5074, -0.1278, 50.0
RDF_TYPE = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type"

TRIPLE = re.compile(r'^\s*(\S+)\s+(\S+)\s+(.+?)\s*\.\s*$')


def local(iri):
    return re.split(r'[#/]', iri.rstrip("#/"))[-1]


def parse_nt(path):
    """Minimal N-Triples parse -> list of (s_iri, p_iri, object) where object is
    ('iri', val) or ('lit', val)."""
    out = []
    for line in open(path, encoding="utf-8"):
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        m = TRIPLE.match(line)
        if not m:
            continue
        s, p, o = m.group(1), m.group(2), m.group(3)
        s = s.strip("<>")
        p = p.strip("<>")
        if o.startswith("<"):
            out.append((s, p, ("iri", o[1:o.index(">")])))
        elif o.startswith('"'):
            out.append((s, p, ("lit", o[1:].split('"')[0])))
        # blank nodes ignored for this sample
    return out


def project(path, outdir):
    """Project triples into typed property-graph CSVs; return the resource->type map."""
    triples = parse_nt(path)
    typ, prop = {}, {}            # uri -> type local name ; uri -> {key: value}
    about, broader = [], []       # (cw, target) ; (concept, parent)
    for s, p, (kind, val) in triples:
        if p == RDF_TYPE and kind == "iri":
            typ[s] = local(val)
        elif local(p) == "about" and kind == "iri":
            about.append((s, val))
        elif local(p) == "broader" and kind == "iri":
            broader.append((s, val))
        elif kind == "lit":
            prop.setdefault(s, {})[local(p)] = val

    def write(name, header, rows):
        with open(os.path.join(outdir, name), "w", newline="") as f:
            w = csv.writer(f, delimiter="|")
            w.writerow(header)
            w.writerows(rows)

    cw = [u for u, t in typ.items() if t == "CreativeWork"]
    place = [u for u, t in typ.items() if t == "Place"]
    concept = [u for u, t in typ.items() if t == "Concept"]
    write("cw.csv", ["uri", "label", "content"],
          [[u, prop.get(u, {}).get("label", ""), prop.get(u, {}).get("content", "")] for u in cw])
    write("place.csv", ["uri", "label", "lat", "lon"],
          [[u, prop.get(u, {}).get("label", ""), prop[u]["lat"], prop[u]["long"]] for u in place])
    write("concept.csv", ["uri", "label"],
          [[u, prop.get(u, {}).get("label", "")] for u in concept])
    write("aboutplace.csv", ["from", "to"], [(a, b) for a, b in about if typ.get(b) == "Place"])
    write("aboutconcept.csv", ["from", "to"], [(a, b) for a, b in about if typ.get(b) == "Concept"])
    write("broader.csv", ["from", "to"], broader)
    return len(cw), len(place), len(concept), len(about), len(broader)


def haversine(latcol, loncol):
    """Cypher great-circle-distance (km) expression from query point to a row."""
    return (f"6371.0088*2*asin(sqrt("
            f"sin(radians({latcol}-({QLAT}))/2)*sin(radians({latcol}-({QLAT}))/2)+"
            f"cos(radians({QLAT}))*cos(radians({latcol}))*"
            f"sin(radians({loncol}-({QLON}))/2)*sin(radians({loncol}-({QLON}))/2)))")


def main():
    tmp = tempfile.mkdtemp()
    counts = project(SAMPLE, tmp)
    print(f"Projected: {counts[0]} works, {counts[1]} places, {counts[2]} concepts, "
          f"{counts[3]} about, {counts[4]} broader edges")

    conn = kuzu.Connection(kuzu.Database(os.path.join(tmp, "db")))
    try:
        conn.execute("INSTALL fts; LOAD fts;")
    except Exception:
        pass
    schema = [
        "CREATE NODE TABLE CreativeWork(uri STRING, label STRING, content STRING, PRIMARY KEY(uri))",
        "CREATE NODE TABLE Place(uri STRING, label STRING, lat DOUBLE, lon DOUBLE, PRIMARY KEY(uri))",
        "CREATE NODE TABLE Concept(uri STRING, label STRING, PRIMARY KEY(uri))",
        "CREATE REL TABLE aboutPlace(FROM CreativeWork TO Place)",
        "CREATE REL TABLE aboutConcept(FROM CreativeWork TO Concept)",
        "CREATE REL TABLE broader(FROM Concept TO Concept)",
    ]
    for s in schema:
        conn.execute(s)
    t0 = time.perf_counter()
    for tbl, f in [("CreativeWork", "cw"), ("Place", "place"), ("Concept", "concept"),
                   ("aboutPlace", "aboutplace"), ("aboutConcept", "aboutconcept"), ("broader", "broader")]:
        conn.execute(f"COPY {tbl} FROM '{tmp}/{f}.csv' (HEADER=true, DELIM='|')")
    conn.execute("CALL CREATE_FTS_INDEX('CreativeWork', 'cwFts', ['content'])")
    load_ms = (time.perf_counter() - t0) * 1000

    # Hybrid (b) pre-filter + correctness reference, from the rustychickpeas run.
    rust = {}
    if os.path.exists("results/spb.rust.json"):
        rust = json.load(open("results/spb.rust.json"))
    near_places = rust.get("geo_places_near_london", [])
    near_list = "[" + ",".join(f"'{u}'" for u in near_places) + "]"

    hav = haversine("p.lat", "p.lon")
    queries = {
        "Q1 about-entity": (
            "MATCH (w:CreativeWork)-[:aboutPlace]->(p:Place {label:'London'}) RETURN DISTINCT w.uri AS uri",
            "about_london"),
        "Q2 category rollup": (
            "MATCH (c:Concept)-[:broader*0..5]->(:Concept {label:'Sport'}) "
            "OPTIONAL MATCH (w:CreativeWork)-[:aboutConcept]->(c) "
            "RETURN c.label AS cat, count(DISTINCT w) AS works", None),
        "Q3 fts (BM25 index)": (
            "CALL QUERY_FTS_INDEX('CreativeWork','cwFts','football') RETURN node.uri AS uri ORDER BY score DESC",
            "fts_football"),
        "Q4a geo Haversine scan": (
            f"MATCH (w:CreativeWork)-[:aboutPlace]->(p:Place) WITH w, {hav} AS km "
            f"WHERE km <= {RADIUS} RETURN DISTINCT w.uri AS uri", "geo_near_london"),
        "Q4b geo hybrid (our k-d tree)": (
            f"MATCH (w:CreativeWork)-[:aboutPlace]->(p:Place) WHERE p.uri IN {near_list} "
            f"RETURN DISTINCT w.uri AS uri", "geo_near_london"),
        "Q5a geo+fts Haversine": (
            f"CALL QUERY_FTS_INDEX('CreativeWork','cwFts','tennis') WITH node AS w "
            f"MATCH (w)-[:aboutPlace]->(p:Place) WITH w, {hav} AS km "
            f"WHERE km <= {RADIUS} RETURN DISTINCT w.uri AS uri", "geo_fts_tennis"),
        "Q5b geo+fts hybrid": (
            f"CALL QUERY_FTS_INDEX('CreativeWork','cwFts','tennis') WITH node AS w "
            f"MATCH (w)-[:aboutPlace]->(p:Place) WHERE p.uri IN {near_list} "
            f"RETURN DISTINCT w.uri AS uri", "geo_fts_tennis"),
    }

    print(f"\nKùzu SPB (load + FTS index in {load_ms:.1f} ms)\n")
    print(f"{'query':<32}{'kùzu ms':>9}  {'rows':>5}  cross-check")
    for name, (cy, ref_key) in queries.items():
        samples = []
        for _ in range(RUNS):
            t = time.perf_counter()
            df = conn.execute(cy).get_as_df()
            samples.append((time.perf_counter() - t) * 1000)
        med = statistics.median(samples)
        if "uri" in df.columns:
            got = sorted(df["uri"].tolist())
            check = ""
            if ref_key and ref_key in rust:
                check = "match" if got == sorted(rust[ref_key]) else f"DIFF (rust={sorted(rust[ref_key])})"
            print(f"{name:<32}{med:>9.3f}  {len(got):>5}  {check}")
        else:
            total = int(df["works"].sum())
            print(f"{name:<32}{med:>9.3f}  {len(df):>5}  rollup total works={total}")


if __name__ == "__main__":
    main()

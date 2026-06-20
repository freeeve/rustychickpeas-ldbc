#!/usr/bin/env python3
"""Kùzu reference side for the SPB queries, on the REAL SPB vocabulary — the
head-to-head against rustychickpeas's hand-coded SPB family.

Projects a self-contained SPB N-Triples extract (creative works + geonames
Features, from scripts/spb_extract.sh) into a Kùzu property graph, builds a BM25
full-text index over title+description, and runs:

  q8 full-text  -- Kùzu `fts` index (BM25)            <- the fts race
  q6 geo        -- Cypher Haversine over Features (no spatial index)
  q6 AND q8     -- composition

then cross-checks the work-uri sets against results/spb.rust.json (emitted by the
`spb` binary on the same extract). Run `target/release/spb <extract>` first.

Usage: .venv-kuzu/bin/python kuzu/run_spb.py <extract.nt> [runs]
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

EXTRACT = sys.argv[1] if len(sys.argv) > 1 else "data/spb/extract/spb-validate.nt"
RUNS = int(sys.argv[2]) if len(sys.argv) > 2 else 5
CW = "http://www.bbc.co.uk/ontologies/creativework/"
GEO = "http://www.geonames.org/ontology#"
WGS = "http://www.w3.org/2003/01/geo/wgs84_pos#"
RDF_TYPE = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type"
TRIPLE = re.compile(r"^\s*<([^>]*)>\s+<([^>]*)>\s+(.+?)\s*\.\s*$")


def lit(o):
    return o.split('"')[1] if o.startswith('"') else None


def clean(s):
    """Strip CSV-hostile chars (the '|' delimiter, quotes, control chars) from a
    display/FTS string. Geonames names/titles can contain '|'."""
    return re.sub(r'[|"\t\r\n]', " ", s) if s else s


def project(path, outdir):
    """Parse the extract into CreativeWork / Feature node CSVs + a mentions
    rel CSV. Returns (n_cw, n_feat, n_mentions)."""
    cw = {}     # uri -> [title, description]
    feat = {}   # uri -> [name, lat, lon]
    is_cw, is_feat = set(), set()
    mentions = []
    for line in open(path, encoding="utf-8"):
        m = TRIPLE.match(line)
        if not m:
            continue
        s, p, o = m.group(1), m.group(2), m.group(3)
        if p == RDF_TYPE:
            if o == f"<{CW}CreativeWork>":
                is_cw.add(s)
            elif o == f"<{GEO}Feature>":
                is_feat.add(s)
        elif p == f"{CW}title":
            cw.setdefault(s, ["", ""])[0] = lit(o) or ""
        elif p == f"{CW}description":
            cw.setdefault(s, ["", ""])[1] = lit(o) or ""
        elif p == f"{CW}mentions" and o.startswith("<"):
            mentions.append((s, o[1:o.index(">")]))
        elif p == f"{GEO}name":
            feat.setdefault(s, ["", None, None])[0] = lit(o) or ""
        elif p == f"{WGS}lat":
            feat.setdefault(s, ["", None, None])[1] = lit(o)
        elif p == f"{WGS}long":
            feat.setdefault(s, ["", None, None])[2] = lit(o)

    def write(name, header, rows):
        with open(os.path.join(outdir, name), "w", newline="") as f:
            w = csv.writer(f, delimiter="|")
            w.writerow(header)
            w.writerows(rows)

    write("cw.csv", ["uri", "title", "description"],
          [[u, clean(cw.get(u, ["", ""])[0]), clean(cw.get(u, ["", ""])[1])] for u in is_cw])
    feats = [u for u in is_feat if feat.get(u, ["", None, None])[1] is not None]
    write("feat.csv", ["uri", "name", "lat", "lon"],
          [[u, clean(feat[u][0]), feat[u][1], feat[u][2]] for u in feats])
    fset = set(feats)
    write("mentions.csv", ["from", "to"],
          [(a, b) for a, b in mentions if a in is_cw and b in fset])
    return len(is_cw), len(feats), sum(1 for a, b in mentions if a in is_cw and b in fset)


def haversine(latcol, loncol, qlat, qlon):
    return (f"6371.0088*2*asin(sqrt("
            f"sin(radians({latcol}-({qlat}))/2)*sin(radians({latcol}-({qlat}))/2)+"
            f"cos(radians({qlat}))*cos(radians({latcol}))*"
            f"sin(radians({loncol}-({qlon}))/2)*sin(radians({loncol}-({qlon}))/2)))")


def main():
    rust = json.load(open("results/spb.rust.json")) if os.path.exists("results/spb.rust.json") else {}
    word = rust.get("word", "football")
    qlat, qlon, km = rust.get("lat", 51.5074), rust.get("lon", -0.1278), rust.get("km", 50.0)

    tmp = tempfile.mkdtemp()
    n_cw, n_feat, n_men = project(EXTRACT, tmp)
    print(f"Projected: {n_cw} CreativeWorks, {n_feat} Features, {n_men} mentions rels")

    conn = kuzu.Connection(kuzu.Database(os.path.join(tmp, "db")))
    try:
        conn.execute("INSTALL fts; LOAD fts;")
    except Exception:
        pass
    for stmt in [
        "CREATE NODE TABLE CreativeWork(uri STRING, title STRING, description STRING, PRIMARY KEY(uri))",
        "CREATE NODE TABLE Feature(uri STRING, name STRING, lat DOUBLE, lon DOUBLE, PRIMARY KEY(uri))",
        "CREATE REL TABLE mentions(FROM CreativeWork TO Feature)",
    ]:
        conn.execute(stmt)
    t0 = time.perf_counter()
    for tbl, f in [("CreativeWork", "cw"), ("Feature", "feat"), ("mentions", "mentions")]:
        conn.execute(f"COPY {tbl} FROM '{tmp}/{f}.csv' (HEADER=true, DELIM='|')")
    conn.execute("CALL CREATE_FTS_INDEX('CreativeWork', 'cwFts', ['title', 'description'])")
    print(f"Kùzu load + FTS index: {(time.perf_counter()-t0)*1000:.0f} ms\n")

    hav = haversine("f.lat", "f.lon", qlat, qlon)
    queries = {
        "q8 fts (BM25 index)": (
            f"CALL QUERY_FTS_INDEX('CreativeWork','cwFts','{word}') RETURN node.uri AS uri", "q8_fulltext"),
        "q6 geo (Haversine scan)": (
            f"MATCH (w:CreativeWork)-[:mentions]->(f:Feature) WITH w, {hav} AS km "
            f"WHERE km <= {km} RETURN DISTINCT w.uri AS uri", "q6_geo"),
        "q6 AND q8": (
            f"CALL QUERY_FTS_INDEX('CreativeWork','cwFts','{word}') WITH node AS w "
            f"MATCH (w)-[:mentions]->(f:Feature) WITH w, {hav} AS km "
            f"WHERE km <= {km} RETURN DISTINCT w.uri AS uri", "q6_q8"),
    }

    print(f"{'query':<26}{'kùzu ms':>9}  {'rows':>6}  cross-check vs rustychickpeas")
    for name, (cy, key) in queries.items():
        samples = []
        for _ in range(RUNS):
            t = time.perf_counter()
            df = conn.execute(cy).get_as_df()
            samples.append((time.perf_counter() - t) * 1000)
        got = sorted(df["uri"].tolist())
        check = ""
        if key in rust:
            check = "MATCH" if got == sorted(rust[key]) else f"DIFF (rust={len(rust[key])})"
        print(f"{name:<26}{statistics.median(samples):>9.2f}  {len(got):>6}  {check}")


if __name__ == "__main__":
    main()

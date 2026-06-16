#!/usr/bin/env python3
"""Independent cross-check of the rustychickpeas SPB query results.

Parses the same SPB N-Triples extract and recomputes, with separate Python code,
the two real-vocabulary queries the engine ran:

  q8 (full-text) — creative works with `word` as a whole token in title/description
  q6 (geo)       — creative works `mentions`-linked to a Feature within km (Haversine)

then diffs against results/spb.rust.json (emitted by the `spb` binary). This is
the SPB analogue of kuzu/compare.py: an independent reference, not the engine's
own code path.

Usage: spb_crosscheck.py <extract.nt> [results/spb.rust.json]
"""
import json
import math
import re
import sys

NT = sys.argv[1]
RJSON = sys.argv[2] if len(sys.argv) > 2 else "results/spb.rust.json"

CW = "http://www.bbc.co.uk/ontologies/creativework/"
WGS = "http://www.w3.org/2003/01/geo/wgs84_pos#"
RDF_TYPE = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type"
TRIPLE = re.compile(r"^\s*<([^>]*)>\s+<([^>]*)>\s+(.+?)\s*\.\s*$")

is_cw = set()
text = {}      # work uri -> list of lowercased title/description strings
mentions = {}  # work uri -> set of feature uris
feat = {}      # feature uri -> [lat, lon]

for line in open(NT, encoding="utf-8"):
    m = TRIPLE.match(line)
    if not m:
        continue
    s, p, o = m.group(1), m.group(2), m.group(3)
    if p == RDF_TYPE and o == f"<{CW}CreativeWork>":
        is_cw.add(s)
    elif p in (f"{CW}title", f"{CW}description") and o.startswith('"'):
        text.setdefault(s, []).append(o.split('"')[1].lower())
    elif p == f"{CW}mentions" and o.startswith("<"):
        mentions.setdefault(s, set()).add(o[1:o.index(">")])
    elif p == f"{WGS}lat" and o.startswith('"'):
        feat.setdefault(s, [None, None])[0] = float(o.split('"')[1])
    elif p == f"{WGS}long" and o.startswith('"'):
        feat.setdefault(s, [None, None])[1] = float(o.split('"')[1])

rust = json.load(open(RJSON))
word, lat, lon, km = rust["word"], rust["lat"], rust["lon"], rust["km"]

wre = re.compile(r"\b" + re.escape(word.lower()) + r"\b")
q8_ref = {u for u in is_cw if any(wre.search(t) for t in text.get(u, []))}


def haversine(a, b, c, d):
    r = 6371.0088
    p1, p2 = math.radians(a), math.radians(c)
    dla, dlo = math.radians(c - a), math.radians(d - b)
    h = math.sin(dla / 2) ** 2 + math.cos(p1) * math.cos(p2) * math.sin(dlo / 2) ** 2
    return 2 * r * math.asin(math.sqrt(h))


near = {u for u, (fa, fo) in feat.items() if fa is not None and fo is not None and haversine(lat, lon, fa, fo) <= km}
q6_ref = {u for u in is_cw if mentions.get(u, set()) & near}


def check(name, ref, got):
    got = set(got)
    ok = ref == got
    print(f"  {name:<16} {'MATCH' if ok else 'DIFF'}  (python_ref={len(ref)}  rust={len(got)})")
    if not ok:
        print(f"      only in ref:  {sorted(ref - got)[:5]}")
        print(f"      only in rust: {sorted(got - ref)[:5]}")
    return ok


print(f"Cross-check vs independent Python reference (word='{word}', {km}km of {lat},{lon}):")
ok = check("q8 full-text", q8_ref, rust["q8_fulltext"])
ok &= check("q6 geo", q6_ref, rust["q6_geo"])
print("RESULT:", "PASS" if ok else "FAIL")
sys.exit(0 if ok else 1)

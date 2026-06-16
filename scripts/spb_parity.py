#!/usr/bin/env python3
"""Oxigraph parity check for the full SPB query suite.

Reads results/spb.parity.rust.json (emitted by the `spb_parity` binary: one
parameter set + every query's full result set), runs the matching adapted SPARQL
from scripts/spb_parity_sparql/<query>.rq against the extract-loaded Oxigraph
store, and diffs the two as order-insensitive sets (uris) or tuple-sets
((key,count) aggregates). URIs are percent-decoded on both sides so the extract's
mixed IRI encoding (works.nt percent-encodes non-ASCII, entities.nt does not)
does not cause spurious diffs. See tasks/052.

Usage: scripts/spb_parity.py [store] [results/spb.parity.rust.json]
"""
import json
import os
import subprocess
import sys
import urllib.parse

STORE = sys.argv[1] if len(sys.argv) > 1 else "data/spb/oxigraph-extract"
RJSON = sys.argv[2] if len(sys.argv) > 2 else "results/spb.parity.rust.json"
SPARQL_DIR = "scripts/spb_parity_sparql"

# rust-json param name -> SPARQL template placeholder.
PLACEHOLDERS = {
    "word": "WORD", "topic": "TOPIC", "entB": "ENT_B", "category": "CATEGORY",
    "audience": "AUDIENCE", "cwType": "CW_TYPE", "dateFrom": "DATE_FROM",
    "dateTo": "DATE_TO", "lat": "LAT", "lon": "LON", "deviation": "DEV",
    "q2_cw": "Q2_CW",
}
# How many leading TSV columns form the comparison key/value for each kind.
KINDS = {
    "uris": ("uri",), "uri_opt": ("uri",), "kv": ("key", "int"),
    "kvx": ("key", "int", "key"), "day_count": ("key", "int"),
    "who_days": ("uri", "int"),
}


def canon(cell):
    """Normalize one TSV/JSON cell: drop <>, quotes and ^^<type>, and
    percent-decode anything that looks like an http(s) IRI."""
    s = str(cell).strip()
    if s.startswith("<") and s.endswith(">"):
        s = s[1:-1]
    elif s.startswith('"'):
        s = s[1:]
        i = s.rfind('"')
        if i >= 0:
            s = s[:i]
    if s.startswith("http"):
        s = urllib.parse.unquote(s)
    return s


def cell_int(cell):
    if isinstance(cell, int):
        return cell
    s = str(cell).strip()
    if s.startswith('"'):
        s = s[1:s.rfind('"')] if s.rfind('"') > 0 else s[1:]
    return int(s)


def fill(template, params):
    out = template
    for name, ph in PLACEHOLDERS.items():
        out = out.replace("{{" + ph + "}}", str(params.get(name, "")))
    return out


def run_sparql(query):
    """Run a SPARQL SELECT against the store, return rows of TSV columns
    (header dropped)."""
    p = subprocess.run(
        ["oxigraph", "query", "-l", STORE, "--results-format", "tsv"],
        input=query, capture_output=True, text=True,
    )
    if p.returncode != 0:
        raise RuntimeError(p.stderr.strip()[:400])
    lines = [ln for ln in p.stdout.splitlines() if ln != ""]
    return [ln.split("\t") for ln in lines[1:]]  # drop the ?var header


def as_set(rows, shape):
    """Project rows to a frozenset per the kind's column shape."""
    out = set()
    for r in rows:
        key = []
        for i, typ in enumerate(shape):
            key.append(cell_int(r[i]) if typ == "int" else canon(r[i]))
        out.add(key[0] if len(key) == 1 else tuple(key))
    return out


def main():
    data = json.load(open(RJSON))
    params = data["params"]
    print(f"{'query':<6}{'rust':>8}{'oxi':>8}  verdict")
    ok = diff = skip = 0
    for name, q in data["queries"].items():
        kind = q["kind"]
        shape = KINDS[kind]
        rust = as_set([row if isinstance(row, list) else [row] for row in q["rows"]], shape)
        path = os.path.join(SPARQL_DIR, f"{name}.rq")
        if not os.path.exists(path):
            print(f"{name:<6}{len(rust):>8}{'—':>8}  SKIP (no {name}.rq yet)")
            skip += 1
            continue
        try:
            oxi = as_set(run_sparql(fill(open(path).read(), params)), shape)
        except RuntimeError as e:
            print(f"{name:<6}{len(rust):>8}{'ERR':>8}  {e}")
            diff += 1
            continue
        if rust == oxi:
            print(f"{name:<6}{len(rust):>8}{len(oxi):>8}  MATCH")
            ok += 1
        else:
            only_r, only_o = rust - oxi, oxi - rust
            print(f"{name:<6}{len(rust):>8}{len(oxi):>8}  DIFF  +rust={len(only_r)} +oxi={len(only_o)}")
            for s in list(only_r)[:2]:
                print(f"        only-rust: {s}")
            for s in list(only_o)[:2]:
                print(f"        only-oxi : {s}")
            diff += 1
    print(f"\n{ok} match, {diff} diff, {skip} pending")


if __name__ == "__main__":
    main()

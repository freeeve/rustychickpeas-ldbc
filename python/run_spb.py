"""Run the ported SPB queries in Python over the RDF extract, comparing each to the
Rust parity reference (results/spb.parity.rust.json: {params, queries}).

q6/q8 (geo/full-text) and the full-text a20/a21 are deferred (the core geo/
full_text_search indexes aren't exposed in Python). Cross-check key = node uri.

Usage: python run_spb.py [extract.nt]
"""

import json
import os
import statistics
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from spb import loader, queries as sq  # noqa: E402

REPO_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DEFAULT_EXTRACT = os.path.join(REPO_ROOT, "data/spb/extract/spb-validate.nt")
PARITY = os.path.join(REPO_ROOT, "results/spb.parity.rust.json")


def main():
    extract = sys.argv[1] if len(sys.argv) > 1 else DEFAULT_EXTRACT
    ref = json.load(open(PARITY))
    params, refs = ref["params"], ref["queries"]

    print(f"Loading RDF extract from {extract} ...", file=sys.stderr)
    t = time.perf_counter()
    g, s = loader.load_ntriples(extract)
    load_s = time.perf_counter() - t
    uri_map = s["uri_to_node"]

    print("\n=== LDBC SPB (RDF -> property graph) — Python (rustychickpeas) ===")
    print(f"Loaded {s['resources']} resources / {s['triples']} triples in {load_s:.1f}s")

    def uris(nodes):
        return [g.get_property(n, "uri") for n in nodes]

    specs = [
        ("q1", lambda: uris(sq.q1(g, uri_map, params["topic"]))),
        ("a9", lambda: [["max", sq.a9(g)]]),
    ]

    runs = int(os.environ.get("SPB_RUNS", "3"))
    print(f"{'Query':<6}{'Rows':>10}{'Time':>12}  Parity")
    print("-" * 42)
    passed = checked = 0
    for qid, fn in specs:
        result = fn()
        times = []
        for _ in range(runs):
            t = time.perf_counter()
            fn()
            times.append((time.perf_counter() - t) * 1000.0)
        ok = result == refs.get(qid, {}).get("rows")
        checked += 1
        passed += bool(ok)
        print(f"{qid:<6}{len(result):>10}{statistics.median(times):>10.1f}ms  {'ok' if ok else 'FAIL'}")
    print("-" * 42)
    print(f"{passed}/{checked} match the Rust parity reference (load {load_s:.1f}s)")


if __name__ == "__main__":
    main()

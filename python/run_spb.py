"""Run the ported SPB queries in Python over the RDF extract, comparing each to
the Rust parity reference (results/spb.parity.rust.json: {params, queries}) and
printing Python vs Rust per-query timings.

Cross-check key = node uri (or the aggregate rows). q6/q8 (the basic geo/full-text
demo queries) are not in the parity set. The full-text family (a15/a16/a20-a23) and
geo a17 are served by Python replicas of the core inverted index / bbox scan, since
the Python binding does not expose full_text_search / geo.

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
RUST_TIMINGS = "/tmp/spb_rust_timings.txt"

ALL = 1 << 62  # stands in for the Rust usize::MAX (LIMIT disabled)
ENTITY_LABEL = "Thing"  # a5's entity-type restriction (coreconcepts:Thing local name)


def _rust_ms():
    """Parse the Rust per-query median-ms table from a captured spb_parity run."""
    out = {}
    if not os.path.exists(RUST_TIMINGS):
        return out
    for line in open(RUST_TIMINGS):
        parts = line.split()
        if len(parts) >= 2:
            try:
                out[parts[0]] = float(parts[1])
            except ValueError:
                pass
    return out


def main():
    extract = sys.argv[1] if len(sys.argv) > 1 else DEFAULT_EXTRACT
    ref = json.load(open(PARITY))
    p, refs = ref["params"], ref["queries"]
    rust_ms = _rust_ms()

    print(f"Loading RDF extract from {extract} ...", file=sys.stderr)
    t = time.perf_counter()
    g, s = loader.load_ntriples(extract)
    load_s = time.perf_counter() - t
    um = s["uri_to_node"]

    def uris(nodes):
        return [sq._uri(g, n) for n in nodes]

    # qid -> thunk producing the comparable rows (same shape as the parity block).
    specs = [
        ("q1", lambda: uris(sq.q1(g, um, p["topic"]))),
        ("q2", lambda: uris(sq.q2(g, um, p["q2_cw"]))),
        ("q3", lambda: uris(sq.q3(g, um, p["topic"], ALL))),
        ("q4", lambda: uris(sq.q4(g, um, p["topic"], ALL))),
        ("q5", lambda: sq.q5(g, p["cwType"], p["audience"], p["dateFrom"], p["dateTo"])),
        ("q7", lambda: uris(sq.q7(g, p["cwType"], p["dateFrom"], p["dateTo"], p["category"], p["audience"]))),
        ("q9", lambda: sq.q9(g, um, p["q2_cw"], ALL)),
        ("a1", lambda: uris(sq.a1(g, um, "about", p["topic"]))),
        ("a2", lambda: sq.a2(g, um, p["q2_cw"])),
        ("a3", lambda: sq.a3(g, p["dateFrom"], p["dateTo"])),
        ("a4", lambda: sq.a4(g, p["dateFrom"], p["dateTo"], ALL)),
        ("a5", lambda: sq.a5(g, um, ENTITY_LABEL, p["catCompany"], p["category"], ALL)),
        ("a6", lambda: sq.a6(g, um, True, p["audience"], ALL)),
        ("a7", lambda: sq.a7(g, 1, ALL)),
        ("a8", lambda: sq.a8(g, p["cwType"], p["audience"], p["dateFrom"], p["dateTo"])),
        ("a9", lambda: [["max", sq.a9(g)]]),
        ("a10", lambda: sq.a10(g, ALL)),
        ("a13", lambda: sq.a13(g, p["catCompany"], p["category"], ALL)),
        ("a14", lambda: uris(sq.a14(g, um, p["primaryFormat"], p["webDocType"], ALL))),
        ("a15", lambda: uris(sq.a15(g, p["word2"], ALL))),
        ("a16", lambda: sq.a16(g, p["word2"], ALL)),
        ("a17", lambda: uris(sq.a17(g, p["lat"], p["lon"], p["deviation"]))),
        ("a18", lambda: uris(sq.a18(g, p["cwType"], p["dateFrom"], p["dateTo"], ALL))),
        ("a19", lambda: sq.a19(g, p["cwType"], p["audience"], p["dateFrom"], p["dateTo"], ALL)),
        ("a20", lambda: uris(sq.a20(g, p["word"], ALL))),
        ("a21", lambda: uris(sq.a21(g, p["word"], p["category"], p["audience"], None, None, None, None, ALL))),
        ("a22", lambda: uris(sq.a22(g, p["word"], p["category"], p["audience"], None, p["dateFrom"], p["dateTo"], None, ALL))),
        ("a23", lambda: sq.a23(g, p["word"], p["category"], ALL)),
        ("a24", lambda: sq.a24(g, um, p["topic"], p["entB"], None, None)),
        ("a25", lambda: sq.a25(g, um, p["topic"], ALL)),
    ]

    print("\n=== LDBC SPB (RDF -> property graph) — Python (rustychickpeas) ===")
    print(f"Loaded {s['resources']} resources / {s['triples']} triples in {load_s:.1f}s")

    def norm(rows):
        # Parity is by identity (uri / aggregate row), order-insensitive — the Rust
        # reference disables LIMITs and emits in randomized-hash order. Compare as a
        # multiset of canonicalized rows.
        return sorted(json.dumps(r, sort_keys=True) for r in (rows or []))

    runs = int(os.environ.get("SPB_RUNS", "3"))
    print(f"\n{'query':<6}{'rows':>9}{'py ms':>11}{'rust ms':>10}{'x':>8}  parity")
    print("-" * 56)
    passed = checked = 0
    for qid, fn in specs:
        result = fn()
        times = []
        for _ in range(runs):
            t = time.perf_counter()
            fn()
            times.append((time.perf_counter() - t) * 1000.0)
        py = statistics.median(times)
        rust = rust_ms.get(qid)
        want = refs.get(qid, {}).get("rows")
        ok = norm(result) == norm(want)
        checked += 1
        passed += bool(ok)
        ratio = f"{py / rust:>7.0f}" if rust else "      -"
        rstr = f"{rust:>10.3f}" if rust else f"{'-':>10}"
        flag = "ok" if ok else f"FAIL (py {len(result)} vs rust {len(want) if want else 0})"
        print(f"{qid:<6}{len(result):>9}{py:>11.2f}{rstr}{ratio}  {flag}")
    print("-" * 56)
    print(f"{passed}/{checked} match the Rust parity reference (load {load_s:.1f}s)")


if __name__ == "__main__":
    main()

"""Run the FinBench complex reads (CR1-CR12) in Python over a loaded ``raw/`` graph,
picking representative seeds (highest-degree) the same way the Rust bin does, and
reporting per-query timing + result shape.

Correctness is pinned by tests/test_finbench.py (the TCR1-CR12 fixture, same exact
assertions as the Rust tests); FinBench has no SF cross-check emit, so this runner
is timing + result-shape only.

Usage: python run_finbench.py [raw_dir]
"""

import os
import statistics
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from finbench.loader import load_finbench  # noqa: E402
from finbench import queries as fb  # noqa: E402
from rustychickpeas import Direction  # noqa: E402

REPO_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DEFAULT = os.path.join(REPO_ROOT, "data/finbench/raw")
WS, WE = -(2 ** 63), 2 ** 63 - 1
WIN = 90 * 86_400_000  # the 90-day cycle window the Rust bin uses
LIMIT = 10_000


def _max_out_degree(g, label, rel):
    best, best_d = 0, -1
    for n in g.nodes_with_label(label):
        d = g.degree(n, Direction.Outgoing, rel)
        if d > best_d:
            best_d, best = d, n
    return best


def main():
    raw = sys.argv[1] if len(sys.argv) > 1 else DEFAULT
    print(f"Loading FinBench graph from {raw} ...", file=sys.stderr)
    t = time.perf_counter()
    g, _ = load_finbench(raw)
    load_s = time.perf_counter() - t

    print("\n=== LDBC FinBench — Python (rustychickpeas) ===")
    print(f"Loaded {g.node_count()} nodes / {g.relationship_count()} rels in {load_s:.1f}s")

    # --- seeds (mirror src/bin/finbench.rs) ---
    by_deg = sorted(((a, g.degree(a, Direction.Both, "transfer"))
                     for a in g.nodes_with_label("Account")), key=lambda x: -x[1])
    seed = by_deg[0][0] if by_deg else 0
    top500 = [a for a, _ in by_deg[:500]]
    cyc_seed, cyc = next(((a, c) for a in top500
                          for c in [len(fb.transfer_cycles(g, a, 1000.0, WIN))] if c > 0),
                         (seed, 0))
    dst = seed
    for _ in range(4):
        nxt = next((rel.end_node().id()
                    for rel in g.relationships(dst, Direction.Outgoing, ["transfer"])
                    if rel.end_node().id() != seed), None)
        if nxt is None:
            break
        dst = nxt
    seed_person = _max_out_degree(g, "Person", "guarantee")
    card = _max_out_degree(g, "Account", "withdraw")
    loan_seed = _max_out_degree(g, "Loan", "deposit")
    investor = _max_out_degree(g, "Person", "invest")
    owner = _max_out_degree(g, "Person", "own")
    cr1_seed = next((a for a in top500 if fb.cr1(g, a, WS, WE, LIMIT, False)), seed)

    specs = [
        ("CR1", "blocked-medium", lambda: len(fb.cr1(g, cr1_seed, WS, WE, LIMIT, False))),
        ("CR2", "loan-gather", lambda: len(fb.cr2(g, owner, WS, WE, LIMIT, False))),
        ("CR3", "shortest-path", lambda: fb.shortest_transfer_path(g, seed, dst, WS, WE)),
        ("CR4", "transfer-cycles", lambda: len(fb.transfer_cycles(g, cyc_seed, 1000.0, WIN))),
        ("CR5", "exact-trace", lambda: len(fb.cr5(g, owner, WS, WE, LIMIT, "desc"))),
        ("CR6", "withdraw-m2o", lambda: len(fb.cr6(g, card, 0.0, 0.0, WS, WE, LIMIT, "desc"))),
        ("CR7", "in-out-ratio", lambda: fb.cr7(g, seed, 0.0, WS, WE, LIMIT, False)),
        ("CR8", "trace-after-loan", lambda: len(fb.cr8(g, loan_seed, 0.0, WS, WE, LIMIT, "desc"))),
        ("CR9", "laundering", lambda: fb.cr9(g, seed, 0.0, WS, WE, LIMIT, False)),
        ("CR10", "investor-sim", lambda: len(fb.cr10(g, investor, WS, WE))),
        ("CR11", "guarantee-exposure", lambda: round(fb.guarantee_exposure(g, seed_person))),
        ("CR12", "company-transfer", lambda: len(fb.cr12(g, owner, WS, WE, LIMIT, False))),
    ]

    runs = int(os.environ.get("FINBENCH_RUNS", "5"))
    print(f"{'Query':<7}{'Description':<20}{'Result':>16}{'Time':>12}")
    print("-" * 55)
    for qid, label, fn in specs:
        res = fn()
        times = []
        for _ in range(runs):
            t = time.perf_counter()
            fn()
            times.append((time.perf_counter() - t) * 1000.0)
        print(f"{qid:<7}{label:<20}{str(res):>16}{statistics.median(times):>10.1f}ms")
    print("-" * 55)
    print(f"(median of {runs}; load {load_s:.1f}s; correctness pinned by test_finbench.py)")


if __name__ == "__main__":
    main()

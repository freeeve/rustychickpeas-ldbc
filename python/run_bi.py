"""Run the Python LDBC SNB BI queries against an ``initial_snapshot`` directory.

Usage:
    python run_bi.py [snapshot_dir]

Defaults to the SF1 dataset under the repo's ``data/`` directory.
"""

import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import harness  # noqa: E402
import loader  # noqa: E402
from bi import q1  # noqa: E402
from props import days_from_civil  # noqa: E402

REPO_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DEFAULT_SNAPSHOT = os.path.join(
    REPO_ROOT,
    "data/bi-sf1-composite-merged-fk/graphs/csv/bi/composite-merged-fk/initial_snapshot",
)


def main():
    snapshot = sys.argv[1] if len(sys.argv) > 1 else DEFAULT_SNAPSHOT
    if not os.path.isdir(os.path.join(snapshot, "dynamic")):
        sys.exit(f"no 'dynamic' dir under {snapshot}; pass the initial_snapshot path")

    print(f"Loading LDBC messages from {snapshot} ...", file=sys.stderr)
    t = time.perf_counter()
    g, stats = loader.load_messages(snapshot)
    load_s = time.perf_counter() - t
    print("\n=== LDBC SNB BI — Python (rustychickpeas) ===")
    print(
        f"Loaded {g.node_count()} message nodes "
        f"({stats['posts']} posts, {stats['comments']} comments) in {load_s:.1f}s\n"
    )

    cutoff = days_from_civil(2011, 12, 1)
    post_count = stats["posts"]
    rows, total = q1.q1_posting_summary(g, cutoff)
    rows_arrow, total_arrow = q1.q1_posting_summary_arrow(g, cutoff, post_count)
    rows_native, total_native = q1.q1_posting_summary_native(g, cutoff)
    assert (rows, total) == (rows_arrow, total_arrow), "arrow Q1 disagrees with loop Q1"
    assert (rows, total) == (rows_native, total_native), "native Q1 disagrees with loop Q1"
    print(
        f"Q1 posting summary: {len(rows)} groups over {total} messages "
        "before 2011-12-01  (loop == arrow == native: ok)"
    )
    for y, is_comment, cat, n, sum_len in rows[:4]:
        kind = "Comment" if is_comment else "Post"
        avg = sum_len / n if n else 0.0
        print(f"   {y} {kind:<7} lenCat={cat}  count={n}  avgLen={avg:.1f}")

    print("\nTimings:")
    harness.time_query("Q1 posting summary (loop)", 5, lambda: q1.q1_posting_summary(g, cutoff)[0])
    harness.time_query(
        "Q1 posting summary (arrow)",
        5,
        lambda: q1.q1_posting_summary_arrow(g, cutoff, post_count)[0],
    )
    harness.time_query(
        "Q1 posting summary (native Rust)",
        5,
        lambda: q1.q1_posting_summary_native(g, cutoff)[0],
    )


if __name__ == "__main__":
    main()

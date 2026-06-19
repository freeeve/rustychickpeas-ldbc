# 181 — Optimize BI Q15 (weighted path)

Baseline (full SF1, median of 5): Python 1343.7 ms vs Rust 17.8 ms (~76x).
Lead: the interaction-weight build iterates all 1.7M Comments; per-comment graph access.

## Result
1343.7ms -> **~700ms** (~1.9x), exact match. Two native arrays replace the per-comment graph
walks/scans:
1. `roots_via("replyOf", Outgoing)` (cached forest-root array) for the thread root -> drops the
   per-comment Python `root_of` chain walk (1344 -> 870ms).
2. `neighbor_via("hasCreator", Incoming)` (one-hop functional-neighbor array, ~17ms build) for
   the two per-comment creator lookups (cc, pc) -> O(1) memoryview index instead of like-heavy
   incoming-hasCreator scans (870 -> 700ms).

Remaining (~700ms): the 1.7M-comment **Python loop itself** (parent first_neighbor + memoryview
indexes + dict per comment). The graph-access bottlenecks are gone; this is the CPython per-row
floor. Beating it needs a native interaction/weight-map build primitive (process all comments in
Rust) — shared with Q19, the bigger sign-off lever, not a per-lookup tweak.

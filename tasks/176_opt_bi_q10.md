# 176 — Optimize BI Q10 (experts in country)

Baseline (full SF1, median of 5): Python 27.4 ms vs Rust 8.2 ms (~3.3x).
Lead: native `bfs_distances` for the bounded knows BFS; then per-expert message scan with a
per-(expert, tag) distinct-message set, filtered by the class-tag set.
Approach: modest gap. Candidate: skip experts with no class-tagged messages earlier;
profile the per-message hasTag fan-out. Lowish priority at ~27 ms.

## Result
(pending)

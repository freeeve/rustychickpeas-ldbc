# 185 — Optimize BI Q19 (interaction path)

Baseline (full SF1, median of 5): Python 652.8 ms (query only) vs Rust 6.79 ms (~96x).
Plus a one-time interaction-map build: Python ~1.8s vs Rust's (precomputed, excluded from
both timings).
Lead: two things. (1) The query runs a full single-source heap Dijkstra per city1 person
over the interaction-projected knows graph, where Rust runs a bidirectional search per
(p1, p2) pair (meets in the middle, far less work). (2) The 1.7M-comment interaction build
is pure Python (~1.8s), shared with Q15.
Approach: (a) bidirectional Dijkstra per pair (or A*-style early-exit) to match Rust's
shape; (b) the native interaction-map build primitive from task 181 removes the ~1.8s
setup. Biggest BI ratio — good target.

## Result
(pending)

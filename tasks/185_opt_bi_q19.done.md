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
PARTIAL (2026-06-19). Lever (2), the ~1.8s interaction-map build, is DONE: it now uses the native
parallel `fold_via` kernel (`creators = g.neighbor_via("hasCreator", Incoming);
g.fold_via("replyOf", Outgoing, creators)`) — precompute (Q19 map + Q20 weights) 1.8s → 0.2s, exact
6-row match preserved. See task 188.

STILL PENDING — lever (1): the per-city1-person single-source heap Dijkstra (~735ms query). Rust runs
a bidirectional search per (p1,p2) pair. To close the remaining gap, reshape the Python query to a
bidirectional / early-exit Dijkstra per pair (pure-Python query change, no core primitive, no
sign-off needed).

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
DONE (2026-06-19) — both levers, each by reusing a native core primitive (no Python heap Dijkstra):
- Lever (2), the ~1.8s interaction-map build → native parallel `fold_via` kernel
  (`g.fold_via("replyOf", Outgoing, g.neighbor_via("hasCreator", Incoming))`); precompute (Q19 map +
  Q20 weights) 1.8s → ~0.1s. See task 188.
- Lever (1), the per-city1-person heap Dijkstra (~735ms query) → native single-source
  `g.dijkstra(p1, Outgoing, "knows", weights=interaction, base=0.0, prune_missing=True)`, reading the
  resident `PairWeights` (cost 1/interaction, untraversable pairs pruned) inside the kernel with the
  GIL released — no per-rel Python callback. **Query 735ms → 59ms (~12.5x); exact 6-row match;
  20/20 BI parity preserved.**

Shipped (main repo, gated, NOT pushed): fold_via now returns a resident dict-like `PairWeights`;
binding `GraphSnapshot.dijkstra(...)` wraps core `dijkstra`; core `ShortestPaths::into_distances()`
(bulk {node: distance} export). Q19 went from the worst BI ratio (96x) to ~8.6x Rust. Eve signed off
on the primitive exercise (PairWeights residency, baked-reciprocal `1/(w+base)` + kwargs, name
`dijkstra`).

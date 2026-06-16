# 003 — Interactive (IC) queries — DONE (feasible tier)

Hand-coded IC reads in `src/interactive.rs`, run + timed by `src/bin/ic.rs`
(median of 5). Implemented and smoke-checked on SF1 (non-empty, stable):
- IC1 friends-by-name (3-hop `knows` BFS), IC2 recent friend messages,
  IC9 recent friends-of-friends messages, IC13 unweighted shortest path,
  IC14 weighted shortest path (dijkstra + interaction-weight projection).
- short reads IS1 profile, IS2 own recent messages, IS3 friends (+ IS5 helper).

IC1/IC9/IC13 are built on the core `bfs_distances` primitive; IC14 reuses
`dijkstra`. Timings: IC1 4.2ms, IC2 19ms, IC9 173ms, IC13 4ms, IC14 13ms,
IS* sub-ms.

Deferred (need more schema loaded; listed in the binary's output): IC3-IC8,
IC10-IC12 (Forum membership / tag co-occurrence / organisation expansions),
IS4/IS6/IS7. Task 004 (Kùzu IC reference) remains open.

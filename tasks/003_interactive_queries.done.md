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

Also implemented later with no loader change: IC4 (new topics), IC6 (tag
co-occurrence), IC8 (recent replies), IS6 (forum of message, via chain_roots),
IS7 (replies of message).

Deferred (need a loader addition): IC3 (msg->country), IC5 (hasMember join
date), IC7 (likes date), IC10 (birthday/gender), IC11 (workFrom + org
location), IC12 (TagClass subclass), IS4 (message content text).

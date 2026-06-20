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

Loader-backed tier added (loader extended additively — Message->Country rels,
hasMember join-date, likes date, Person birthday/gender, workAt workFrom +
Company location, TagClass subclass rels; BI cross-check re-verified 0-diff):
IC3, IC5, IC7, IC10, IC11, IC12.

IS4 (message content) added behind the loader's opt-in `load_content` flag
(`load_graph_opts(.., true)`): `ctext` is stored alongside the `hasContent` bool
(Post falls back to `imageFile`), so the `ic` binary opts in while BI/SPB loads
stay lean. The full IC1-IC14 + short reads IS1-IS7 workload is now implemented.

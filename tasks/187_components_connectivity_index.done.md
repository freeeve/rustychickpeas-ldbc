# 187 — General components / connectivity index (union-find / WCC-adjacent)

Follow-up from the `roots_via`/`root_via` design discussion (the chain-root read primitive).

## Idea

A general **connectivity index**: `component_of(node)` and `same_component(a, b)`, built once
via union-find (or BFS) over a relationship (set) and materialized as a flat `node -> component
representative` array, with O(1) lookups. Same "node -> representative labeling" shape as
`roots_via`, but for **arbitrary (non-functional) rels**, not just single-successor chains.

Relationship to `roots_via`: `roots_via` (functional relation -> chain terminal) is the special
case where each node has one parent; "same thread?" for replyOf is exactly
`roots_via("replyOf")[a] == roots_via("replyOf")[b]`. This index is the general case.

## Why it's worth it

- **WCC is literally this** — a materialized component index is weakly-connected-components'
  output. So this OVERLAPS the graphalytics WCC work the other session owns. **Coordinate with
  them rather than building a parallel union-find/components implementation.**
- **Q19/Q20 reachability pre-check:** both run Dijkstra between person pairs over `knows`. An
  O(1) `same_component(src, tgt)` lets them skip the whole search when the pair isn't connected —
  a real optimization (union-find's canonical "same set?" query).
- General "are these two nodes connected / in the same cluster" predicates.

## Notes

- For an immutable snapshot + read-heavy access, the optimal form is a materialized flat
  component-id array (union-find is the BUILD algorithm; a live union-find only wins for
  mutation/streaming). Mirror `roots_via`: cache per relationship(-set), expose a buffer-protocol
  array + a singular accessor + a `same_component(a, b)`.
- Could share one "partition / labeling" surface with `roots_via` (`[node] -> representative` +
  `same(a, b)`), since chain-root and component-id are both representative labelings.
- Sign-off + coordination needed (core public API + graphalytics overlap).

## Result
(pending)

## Result (2026-06-21) — DONE (provided by native wcc)
Native `GraphSnapshot::wcc` (analytics.rs, core d4a1d36) returns the materialized
`node -> component representative` (smallest node id) array — exactly this index for
the all-rels/undirected case. `component_of(n) = wcc()[n]`; `same_component(a,b) =
wcc()[a] == wcc()[b]` (O(1) after one build), usable for the Q19/Q20 reachability
pre-check. The remaining generalization (per-arbitrary-rel-set union-find index,
shared labeling surface with roots_via) is deferred — wcc covers the demand seen so
far; revisit if a per-rel-set component index is actually needed.

# 141 — Core: predicate + traversal primitives (Tier-2 simplification)

Core `GraphSnapshot` additions from the review (`rustychickpeas-core`). All are
data-in/out → also expose to Python (tasks/143). Apply when core is clean.

## #4 Facet predicates (SPB ~19 sites; 5 duplicate private helpers)
- `has_edge(n, dir, edge) -> bool`
- `has_neighbor_with_property(n, dir, edge, key, val) -> bool`
Collapses `facet_matches`/`facet_edge`/`has_edge_to_uri`/`has_any_edge` across ~8 SPB
files. `has_label` already exists in core but isn't exposed to Python.

## #5/#6/#9/#10 Traversal + lookups
- `khop_nodes(seed, dir, rel, hops: RangeInclusive) -> NodeSet` (IC 6) — replaces
  `bfs_distances` + distance-filter; `reachable_along(start, &[steps]) -> NodeSet`
  (BI 6, typed path). Return NodeSet for O(1) downstream membership.
- `neighbors_by_types(n, dir, &[edges])` — **deduped** union (SPB ~10, the
  about∪mentions fold). NB: `neighbors_by_type(slice)` already unions but does NOT dedup.
- `descendants(root, dir, rel)` reply-tree iterator (BI 3; complements `chain_roots`).
- `degree(n, dir, edge) -> usize` with an **O(1) CSR-offset** path for the no-filter
  case (SPB ~4; also fixes the Python-only O(degree) `degree` divergence — tasks/143).
- `node_by_label_property(label, key, val) -> Option<NodeId>` (label-scoped sibling
  of `node_by_property`; IC ~6).

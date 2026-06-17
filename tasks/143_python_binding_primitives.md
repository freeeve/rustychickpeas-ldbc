# 143 — Python binding: expose primitives + fix divergences

From the Python-perspective review of `rustychickpeas-python`. Most data-in/out core
primitives (tasks/140-141) should also get Python methods (double ROI).

## Expose (high value / low effort)
- `has_label`, `has_edge`, `has_neighbor_with_property` as bool methods (kills the
  `"Person" in node_labels(n)` per-node label-index scan inside `bfs` filters).
- `node_by_label_property -> Optional[int]` (collapses `nodes_with_property(..)[0]`).
- `first_neighbor -> Optional[int]` / `follow -> list[int]` — return **ids, not Node
  objects** (removes the Vec<Relationship>→Vec<Node> Arc-clone churn).
- Bulk column accessors `i64_column(key) -> np.ndarray|list` (+ scalar `str_prop`),
  built on `Column::as_*_slice`. The zero-copy Rust reader struct can't cross PyO3.
- `khop_nodes`/`reachable_along`/`descendants` -> `list[int]`.

## Fix divergences / bugs
- `degree` is Python-only and O(degree) (`.neighbors().count()` in graph_snapshot.rs +
  node.rs); add the O(1) core path (tasks/141), switch both binding impls, add an
  edge-type filter.
- `GraphSnapshot.neighbor_ids` lacks `rel_types` (Node.neighbor_ids has it); add + dedup.
- `Node.relationships` Incoming/Both does an O(out-degree) reverse-scan (node.rs:87-183)
  — use the existing `in_to_out` map.
- Add a lightweight `NodeSet` pyclass (contains/len/intersect/union) so set-returning
  results compose without round-tripping through Python `set()`.

## Do NOT cross PyO3
Closure forms — `top_k_by(key_fn)`, generic `par_map_into`/`par_group_by` (rayon + GIL
re-acquire serializes), and the date helpers (Python stdlib). Expose only
fused/specialized forms (`top_k_by_property`, `group_count_by_property`).

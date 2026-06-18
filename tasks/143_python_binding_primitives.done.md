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

## Done (core 2114a3b, e732558, 2681325, 8e076ce)
**Expose** (batch 1, `2114a3b`): `has_label`, `has_rel`, `has_neighbor_with_property`,
`node_with_label_property`, `first_neighbor`, `follow`, `prop_str`, `neighborhood`,
`i64_column` — using the renamed core API (`has_rel` not `has_edge`, `prop_str` not
`str_prop`, `neighborhood` not `khop_nodes`).
**Divergence fixes** (batch 2a `2681325` + 2b `8e076ce`): `degree` O(1) from CSR
offsets + typed count (GraphSnapshot + Node); `neighbor_ids` gains a `rel_types`
filter (deduped); `Node.relationships` incoming maps via `in_to_out` (O(1)) with a
reverse-scan fallback for builder graphs; new **`NodeSet` pyclass** (len/contains/iter,
`&`/`|`/`-` + intersect/union/difference).
**Bonus**: pyo3 `0.20 -> 0.26` bump (`e732558`) — extension now builds natively on
Python 3.14 (was capped at 3.12). **210 pytest pass.**
Deferred (per tasks/141): `reachable_along`/`descendants` are BI-specific, not core
primitives. Closure forms (`top_k_by`, `par_map_into`/`par_group_by`) deliberately
NOT crossed PyO3. Outstanding: 12 benign noop-`.clone()` lints in pre-existing filter
closures.

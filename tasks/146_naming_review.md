# 146 — Naming review of this session's new API surface

Review the names added during the tasks/140–145 simplification + holistic adoption
pass with the same scrutiny we applied to `neighborhood` (check each against what
established property-graph / graph APIs call the operation — Neo4j core + APOC,
igraph, NetworkX, Gremlin/TinkerPop, ArangoDB AQL, Kùzu — and the repo's node/rel
convention), and rename where a clearer/standard term exists.

## Decided
- `neighborhood(seed, dir, rel, hops: RangeInclusive) -> NodeSet` — chosen over
  `khop_nodes` / `nodes_within_hops` / `subgraph_nodes` (igraph precedent;
  set-valued name honest about the NodeSet return).
- `has_edge` → `has_rel` (already renamed; node/rel convention).
- `relationship_property` → `rel_prop` (rel analogue of node `prop`; the verbose
  name kept as a thin alias so the in-progress FinBench client keeps compiling).
- `i64_col` / `bool_col` / `i64_rel_col` → `col(key)` / `rel_col(key)` returning a
  resolved `Col`, narrowed by `Col::i64()` / `Col::bool()`. Type-prefixed accessor
  names are un-rusty (Rust leads with the operation, type as a suffixed getter:
  `as_i64`, `get_i64`); the type-free accessor + typed getter is the polars
  `column(name).i64()` shape. `I64Col`/`BoolCol` (+ `get`/`as_slice`) unchanged.

## To scrutinise (core `GraphSnapshot`)
- `first_neighbor`, `follow` (chained single-step walk)
- `node_by_property` / `node_by_label_property`
- `has_rel`, `has_neighbor_with_property`
- `str_prop` (None on absent OR empty)

## To scrutinise (LDBC-side, `props.rs` / `bi`)
- `top_k_by_key` / `top_k_by_count`; `TopK` (`push` / `into_sorted_desc`)
- `col_i64` / `col_bool` (BI helpers over the readers)
- `parse_ymd` / `parse_date` / `parse_ms` / `days_from_civil`

## Method
For each: name the operation as the major engines/libraries do, note divergences,
pick the clearest. Apply renames atomically (core rename + client call-sites + a
verification run) like the `has_edge`→`has_rel` change.

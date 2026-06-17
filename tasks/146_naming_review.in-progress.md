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

## Rename set 1 — clear rusty wins (do these first)

Each breaks a Rust convention, not just taste. Apply atomically (core rename +
client call-sites + verification) like `has_edge`→`has_rel`.

1. **`NodeSet::new(RoaringBitmap)` + `new_bitset(BitVec)` → `From` impls.** `new` is
   the canonical/empty ctor in Rust (`Vec`/`String`/`HashMap`/`RoaringBitmap::new`
   are all empty); representation-wrapping is `From`'s job, and `new_<variant>` is a
   shape std never uses. Add `impl From<RoaringBitmap> for NodeSet`,
   `impl From<BitVec> for NodeSet`, `impl Default` (= empty). Two distinct source
   types ⇒ `From` is unambiguous (no need for `from_roaring`/`from_bitset`). Keep
   `empty()` as the readable spelling; optionally make `new()` the no-arg empty.
   Call-site sweep in both repos (`NodeSet::new(x)` → `NodeSet::from(x)` / `x.into()`).
2. **`str_prop` → `prop_str`** (core). Same type-prefix smell as `i64_col`;
   operation-then-type-suffix groups under `prop`, matches `as_str`/`get_i64`. Keeps
   the empty→`None` semantics. Pure rename.
3. **`node_by_property` / `node_by_label_property` →
   `node_with_property` / `node_with_label_property`** (core). Aligns with the
   existing `_with_property` family (`nodes_with_property`,
   `has_neighbor_with_property`); `_by_` connotes "by a closure" in Rust
   (`sort_by_key`/`max_by`).
4. **`fts` → `full_text_search`** (core; `text_search` acceptable). A bare DB
   acronym in a public method is the un-rusty bit; std/ecosystem spell things out.

## Keep — already idiomatic (no change)
- `bfs` / `bfs_distances` / `dijkstra` — petgraph spells them the same (`visit::Bfs`,
  `algo::dijkstra`); abbreviation OK because it's the standard algorithm name.
- `first_neighbor` — mirrors slice `<[T]>::first()` (first elem → `Option`).
- `has_rel` / `has_neighbor_with_property` — `has_`-predicate family; verbose because
  the operation is, not the name.
- `days_from_civil` — literal name of Hinnant's date algorithm (deliberate citation);
  `parse_date` / `parse_ymd` / `parse_ms` — fine verb-first parse names.
- `TopK::push` (matches `Vec`/`BinaryHeap`), `into_sorted_desc` (`into_` consuming
  conversion).

## Optional — weak / judgment (revisit AFTER rename set 1)
- `pstr` / `pi64` / `pbool` (LDBC) → `prop_str` / `prop_i64` / `prop_bool`. Cryptic
  abbreviations, but they're terse hot-loop locals that also **default**
  (`pi64`→0, `pbool`→false), so not pure renames of core `prop_str`. Fold in only for
  full property-read uniformity; large call-site churn. Medium.
- `follow` → `follow_path` — clearer it walks a fixed path of steps. Weak.
- `TopK::new(k)` → `with_limit(k)` — `new(k)` doesn't say what `k` is. Weak.
- `col_i64` / `col_bool` (BI) → `i64_or_zero` / `bool_or_false` — honest about the
  `0`/`false` fallback now that core owns `col().i64()`. Weak.
- `top_k_by_key` — `_by_key` without a closure mildly clashes with `sort_by_key`;
  acceptable as-is.

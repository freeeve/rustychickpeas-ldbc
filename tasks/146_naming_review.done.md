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

## Rename set 1 — clear rusty wins ✅ DONE

Applied + verified (core 304 tests, ldbc 113 lib tests, all bins build). Core part:
commit `cf2d48b`. LDBC call-sites: folded into `d15180b` (the concurrent finbench
session's commit swept up my staged files — we share one working tree + git index;
use `git worktree` to isolate next time). Each broke a Rust convention, not taste:

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

## Weak / judgment — reviewed ✅ (1 applied, 4 kept)
- **`col_i64` / `col_bool` (BI) → `i64_or_zero` / `bool_or_false`** — APPLIED
  (`657e189`). Reads honestly (falls back to 0/false, like `unwrap_or`) and stops
  colliding in spelling with the core `col().i64()` chain.
- `pstr` / `pi64` / `pbool` (LDBC) — KEEP. `prop_str` would clash with the new core
  `g.prop_str` method (different empty-string semantics); `prop_i64`/`prop_bool`
  wouldn't convey the `0`/`false` defaulting any better than the terse name. Big
  churn, real downsides — not actually more rusty.
- `TopK::new(k)` — KEEP. `new(required_arg)` is idiomatic (`Mutex::new`, `Cell::new`,
  `NonZero::new`); `with_limit` is just more verbose.
- `follow` — KEEP. Terse and the `steps` arg is self-evident; `follow_path` is taste.
- `top_k_by_key` — KEEP. Minor `_by_` connotation, acceptable.

## Status: COMPLETE
Rename set 1 applied (core `cf2d48b`, ldbc folded into `d15180b`); weak set reviewed
(`657e189`). Keeps documented above + in the "Keep — already idiomatic" section.

## Postscript — `prop()` got the `col()` treatment (the consistent finish)
The `col(key).i64()` decision implied the single-value reads should match: `prop_str`
was the type-suffixed odd-one-out. So `prop(node, key)` now returns a narrowable
`Prop` (core `b340204`), narrowed by `Prop::str()` / `.i64()` / `.bool()` / `.f64()`,
with `Prop::value()` for the raw `ValueId` — the polars `…​.i64()` shape, symmetric
with `Col`. LDBC `pstr`/`pi64`/`pbool` now wrap it (`1bd43a8`); `pstr` thereby folds
empty→None like `prop_str` (113 tests confirm no regression).
- Transitional aliases left in core (removable once LDBC fully migrates + sessions are
  in separate worktrees): `prop_str` → `prop().str()`, `relationship_property` →
  `rel_prop`.

### `pstr`/`pi64`/`pbool` → core, then deleted
"If this is such a strong use case (129+ sites), should it be in core?" — yes. Added
`PropExt` (core `380d50c`): an extension trait on `Option<Prop>` so `g.prop(n, k)`
reads directly — `.str()`/`.i64()`/`.bool()`/`.f64()` flatten the `and_then`, and
`.str_or`/`.i64_or`/`.bool_or`/`.f64_or` fold in a default. The LDBC `pstr`/`pi64`/
`pbool` helpers were then **deleted** (`f68c96e`), all ~146 call sites using the core
API (`pstr→.str()`, `pi64→.i64_or(0)`, `pbool→.bool_or(false)`); `props` re-exports
`PropExt` so glob importers get it free. 113 tests, all bins, no new warnings — the
hot comparators read cleanly now (`g.prop(a,"plid").i64_or(0).cmp(&g.prop(b,"plid").i64_or(0))`).

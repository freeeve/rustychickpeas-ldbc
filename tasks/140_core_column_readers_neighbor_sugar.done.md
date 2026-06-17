# 140 — Core: typed column readers + neighbor sugar (Tier-1 simplification)

The two highest-leverage core primitives from the client-simplification review
(land in `rustychickpeas-core/src/graph_snapshot.rs`). **Blocked**: apply when the
other session's `graph_snapshot.rs` working copy is committed/clean.

## #1 Typed column readers (~36 client sites: BI ~20, IC 7, SPB ~9)
Resolve a property key → column once and take the dense `as_i64_slice()` /
`as_bool_slice()` fast path, exposing `.get(node)`:
- `i64_col(key) -> Option<I64Col<'_>>`, `bool_col`, `i64_edge_col` (rel cols, by CSR pos).
- `str_prop(node, key) -> Option<&str>` — **None on absent OR empty** dense column
  (kills the copy-pasted `pstr(..).filter(|s| !s.is_empty())` footgun).
Retires the LDBC `col_i64` helper (21 sites) + the `day_col`/`day_s`/`day_of` closure
triad; removes a latent O(n log n) per-comparison column re-resolve in IC7/10/11 and
BI plid comparators. Build on the existing `Column::as_*_slice`.

## #3 first_neighbor / follow (~26 client sites: IC 12+, BI 14+)
- `first_neighbor(n, dir, rel) -> Option<NodeId>` (the ubiquitous `.next()` idiom).
- `follow(n, &[(dir, rel)…]) -> Option<NodeId>` (chained one-of-each-step walks,
  e.g. person→city→country); lets BI drop ~6 re-defined `creator_of` closures.

Both are data-in/out → also expose to Python (tasks/143).

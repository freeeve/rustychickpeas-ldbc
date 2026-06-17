# 142 — LDBC client simplification (adopt primitives + helpers)

Migrate the benchmark clients (`rustychickpeas-ldbc/src`) onto existing + new
primitives. Avoid `src/interactive.rs` while the other (IC) session owns it.

## Tier 0 — adopt EXISTING core primitives (pure deletion)
- `node_by_property` for the `nodes_with_label(L).iter().find(pstr/pi64==x)` scans
  (BI 11+, IC). Verify label-uniqueness; where label-scoped, use
  `node_by_label_property` (tasks/141) instead.
  - DONE (BI): `place_by_lid` (also drops its 2-label City/Country scan) and
    `person_by_plid` now call `node_by_property` — `lid`/`plid` are globally unique
    and type-exclusive. Q19/Q20 unchanged on SF1.
  - BLOCKED (BI): the `name`-keyed finds — `org_by_name`, and the Country / Tag /
    TagClass `.find(name == x)` sites in faithful_a/b/c. `name` is shared across
    labels (a Tag and a Country can collide), so a global `node_by_property` is
    unsafe; these need `node_by_label_property` (tasks/141). ~9 sites.
- `neighbors_in_set` for the "iterate hasCreator, keep only Posts" filter (IC ~5).

## #2 — finish the top-k migration
SPB done: `top_k_by_key` (generalized to any `Ord` id) + `top_k_by_count` now back
every limit-bearing ranked query — q3/q4/a5/a14/a20 (node id) and a4/a6/a23 (String
id from label/uri grouping). No `sort_by(...)+truncate` boilerplate remains in SPB.
Deliberate non-fits left as-is: a3/q5 rank *all* rows (no limit), a19 is a 3-key
sort with a carried date payload.
BI done: Q7 (`q7_related_topics`, top 100) and Q17 (`q17_information_propagation`,
top 10) — the only two BI ranked sites with the 2-tuple `(id, count desc / id asc)`
shape — now use `top_k_by_key`. The other BI sort+truncate sites are multi-column
tuples sorted on assorted fields (`b.3`/`b.4`/multi-key blocks) and do NOT fit the
2-tuple helper; left as-is.
Still open: a streaming `TopK<T>` accumulator for the `BinaryHeap<Reverse<…>>` sites
(IC ~9, when IC is free) and a stored-property (plid) tiebreak variant — now
UNBLOCKED, the tasks/140 `i64_col`/`i64_edge_col` readers landed (see Tier 1).

## Tier 1 — adopt the tasks/140 core primitives (NOW UNBLOCKED, core landed)
`tasks/140` shipped in core (`d34c6f9`): `i64_col`/`bool_col`/`i64_edge_col`
(resolve-once typed column readers), `str_prop` (None on absent OR empty), and
`first_neighbor` / `follow`. Client adoption (BI + SPB now; IC when free):
- `str_prop` → the `pstr(g, n, k).filter(|s| !s.is_empty())` sites (SPB ~9: a20,
  a24, q3, … and BI date-modified reads).
- `i64_col` / `i64_edge_col` → retire LDBC `col_i64` (BI ~21, incl. the per-compare
  re-resolve in the plid/date tiebreak comparators — hoist the reader out of the sort).
- `first_neighbor` → the `neighbors_by_type(..).next()` idiom (BI ~14, SPB facets).
- `follow` → BI person→city→country and the re-defined `creator_of` closures (BI ~6).

## #7 — date helpers (props.rs)
`parse_ymd(s) -> Option<(i32,u32,u32)>` — DONE: lifted from a24, the calendar-field
analog of `parse_date`. Keep in LDBC — does NOT belong in core or Python.

`in_window(s, after, before, inclusive)` — DECLINED. The window sites are single
inline comparisons (`dt > after && dt < before`) that split three ways: inclusive
both-bounds (a18, q7), exclusive both-bounds (a3, a4, a8), and inclusive open-ended
`Option` bounds (a21, a24). A bare-`bool` helper is the same length as the inline
compare while hiding the inclusive/exclusive intent behind `true`/`false` — a
readability regression, unlike top-k where the helper removed an error-prone sort +
tie-break. Only genuine dup is a4≡a8's `pstr(..).filter(!empty).is_some_and(window)`
predicate; not enough to motivate the helper. Revisit only if a window site grows
non-trivial.

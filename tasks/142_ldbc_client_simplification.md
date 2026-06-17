# 142 — LDBC client simplification (adopt primitives + helpers)

Migrate the benchmark clients (`rustychickpeas-ldbc/src`) onto existing + new
primitives. Avoid `src/interactive.rs` while the other (IC) session owns it.

## Tier 0 — adopt EXISTING core primitives (pure deletion)
- `node_by_property` for the `nodes_with_label(L).iter().find(pstr/pi64==x)` scans
  (BI 11+, IC). Verify label-uniqueness; where label-scoped, use
  `node_by_label_property` (tasks/141) instead.
- `neighbors_in_set` for the "iterate hasCreator, keep only Posts" filter (IC ~5).

## #2 — finish the top-k migration
SPB done: `top_k_by_key` (generalized to any `Ord` id) + `top_k_by_count` now back
every limit-bearing ranked query — q3/q4/a5/a14/a20 (node id) and a4/a6/a23 (String
id from label/uri grouping). No `sort_by(...)+truncate` boilerplate remains in SPB.
Deliberate non-fits left as-is: a3/q5 rank *all* rows (no limit), a19 is a 3-key
sort with a carried date payload.
Still open: a streaming `TopK<T>` accumulator for the `BinaryHeap<Reverse<…>>` sites
(IC ~9, when IC is free) and a stored-property (plid) tiebreak variant once column
readers (tasks/140) land. BI ranked sites still to sweep.

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

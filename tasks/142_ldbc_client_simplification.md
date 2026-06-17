# 142 — LDBC client simplification (adopt primitives + helpers)

Migrate the benchmark clients (`rustychickpeas-ldbc/src`) onto existing + new
primitives. Avoid `src/interactive.rs` while the other (IC) session owns it.

## Tier 0 — adopt EXISTING core primitives (pure deletion)
- `node_by_property` for the `nodes_with_label(L).iter().find(pstr/pi64==x)` scans
  (BI 11+, IC). Verify label-uniqueness; where label-scoped, use
  `node_by_label_property` (tasks/141) instead.
- `neighbors_in_set` for the "iterate hasCreator, keep only Posts" filter (IC ~5).

## #2 — finish the top-k migration
`top_k_by_key` is in `props.rs` and used by q4 (done). Migrate the remaining
date/score sort-then-truncate sites (SPB q1/q3/a1/a14/a18/a20…). Add a streaming
`TopK<T>` accumulator for the `BinaryHeap<Reverse<…>>` sites (IC ~9, when IC is free)
and a stored-property (plid) tiebreak variant once column readers (tasks/140) land.

## #7 — date helpers (props.rs)
`parse_ymd(s) -> Option<(i32,u32,u32)>` (retires a24::ymd) and
`in_window(s, after, before, inclusive)` (unifies the ad-hoc inclusive/exclusive
window logic; SPB ~11). Keep these in LDBC — they do NOT belong in core or Python.

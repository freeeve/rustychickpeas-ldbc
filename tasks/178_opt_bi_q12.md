# 178 — Optimize BI Q12 (message histogram)

Baseline (full SF1, median of 5): Python 946 ms vs Rust 38.8 ms (~24x).
Lead: a full 2.8M-message scan. day/len/content already read via dense-column `memoryview`
(O(1), C-speed), but the Python `for`-loop over 2.8M nodes is itself the floor (~0.5-1s).
The root-post-language check (memoized replyOf walk) and creator attribution only run for
the filtered subset.
Approach: (a) numpy-vectorize the day/len/content mask — but numpy isn't in the venv (would
add a dep); (b) a native columnar filter that returns the surviving message ids, then do
only the root-lang/creator work in Python; (c) push the whole histogram to the `aggregate`
kernel — blocked by the per-message root-post-language graph traversal, which the kernel
can't express. (b) is the most promising native route.

## Deferred design (2026-06-20, paused for sign-off — task stays OPEN)
Settled route, pending the primitive-exercise sign-off: extend the core `aggregate` kernel with a
**projected-property filter** so option (c) is unblocked (no pyarrow/numpy — see repo CLAUDE.md /
[[no-pyarrow-numpy-for-queries]]). aggregate already does the scalar filters (day/content/len) +
`through(hasCreator)` + parallel `par_fold`; the ONLY gap is the root-post-language check on a
*projected* node.

New API (count membership, declarative, no callback):
- Python `Aggregation.where_via(projection, column, values)` / Rust `filter_via(&projection, column,
  &allowed_value_ids)` — keep sources whose projected node's `column` value ∈ `values`. Kernel reads
  `col.get(proj[i])` and tests set membership (any column type, sparse-safe); applied AFTER the scalar
  filters so only survivors pay it. `projection` = a `roots_via("replyOf", Out)` NodeArray.

Q12 then =
```
roots = g.roots_via("replyOf", Direction.Outgoing)
rows = (g.aggregate("Post","Comment")
         .where("day",">",min_day).where("content","==",1).where("len","<",len_thr)
         .where_via(roots, "lang", langs)
         .through("hasCreator", Direction.Incoming).run()).rows   # [{neighbor: creator, count: n}]
```
then histogram `[r.count]` + zero-bucket (total_persons - len(rows)) + sort, in Python (cheap).
Expect ~950ms → Rust-class (Rust q12 = 38.8ms single-thread; aggregate is parallel).

OPEN DECISIONS for sign-off (Eve paused the AskUserQuestion to clarify): (1) extend aggregate with
`where_via` vs a standalone `select`→NodeSet + `neighbor_counts`; (2) name `where_via`/`filter_via` vs
`where_projected`. Re-ask when resuming.

## Result
(pending — deferred, NOT done)

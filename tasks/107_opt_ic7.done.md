# 107 — Optimize IC7: recent likers

Per-query optimization loop. Methodology + commands:
[100_ic_perf_methodology.md](100_ic_perf_methodology.md).

## Loop
- [x] 1. Bench allocations: ~20 allocs / ~113 KB (not alloc-bound)
- [x] 2. CPU profile: **the sort dominated** (`driftsort`/`quicksort` ~75%) — collects
  every liker and sorts the lot with a `pi64("plid")` lookup *per comparison*
- [x] 3. Optimize: size-20 heap by (likeDate desc, plid asc); `plid` resolved once
  per liker instead of O(n log n) times
- [x] 4. Re-bench: 20 allocs / 113 KB
- [x] 5. Wall-clock A/B: ~1.19 → ~0.31 ms (~3.8×), same window
- [x] 6. Value-identity: IC emit byte-identical

## Measurements
| metric                  | baseline | after |
|-------------------------|----------|-------|
| allocs                  | ~20      | 20    |
| bytes                   | ~113 KB  | 113,176 |
| wall-clock median (ms)  | ~1.19    | ~0.31 (~3.8×) |
| hot fn (CPU %)          | sort (~75%) | CSR walk |

## Notes

The seed is high-degree (many likers on its messages), so collecting every liker
and sorting them all to take 20 — with a `plid` property lookup per comparison —
was ~75% of the runtime. Replaced with a size-20 min-heap keyed on
(likeDate desc, plid asc); `plid` is resolved once per liker, not per comparison.
Value-identical. (Not "already minimal" despite the small absolute time — the sort
was a real ~3.8× inefficiency.)

**Status: done.**

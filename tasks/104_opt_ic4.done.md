# 104 — Optimize IC4: new topics

Per-query optimization loop. Methodology + commands:
[100_ic_perf_methodology.md](100_ic_perf_methodology.md).

## Loop
- [x] 1. Bench allocations: 36 allocs / 322 KB
- [x] 2. CPU profile: CSR walk ~30%, `get_id`/`pi64("day")` ~15%, `in_window`/`before`
  HashMap+HashSet inserts ~12% — the maps *grow* (doubling reallocs) = the 314 KB
- [x] 3. Optimize: dense tag-count array over the contiguous Tag id range (no hashing,
  no growth) + day-column hoist
- [x] 4. Re-bench: 13 allocs / 157 KB
- [x] 5. Wall-clock A/B: ~7.2 → ~5.3 ms (~27%), same window
- [x] 6. Value-identity: IC emit byte-identical

## Measurements
| metric                  | baseline | after |
|-------------------------|----------|-------|
| allocs                  | 36       | 13    |
| bytes                   | 321,832  | 156,640 |
| wall-clock median (ms)  | ~7.2     | ~5.3 (~27%) |
| hot fn (CPU %)          | balanced (HashMap growth) | CSR walk |

## Notes

(First closed as "already-minimal" on wall-clock — corrected: the allocations
*were* improvable.) `in_window` HashMap + `before` HashSet grew by doubling as
they ingested the friends' post tags (the 314 KB). Tag ids are a contiguous
block (`nodes_with_label("Tag").as_range()`), so tally tag uses in a dense
`Vec<u32>` indexed by `tag - lo`, with a `u32::MAX` sentinel marking tags also
used before the window (excluded from "new"). No hashing, no growth; plus the
IC3-style `day` hoist. Value-identical (id unique; sort makes order deterministic).
Falls back to the HashMap path if Tag ids are ever non-contiguous.

**Status: done** (dense tag array + day hoist).

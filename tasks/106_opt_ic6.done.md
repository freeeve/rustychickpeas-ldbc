# 106 — Optimize IC6: tag co-occurrence

Per-query optimization loop. Methodology + commands:
[100_ic_perf_methodology.md](100_ic_perf_methodology.md).

## Loop
- [x] 1. Bench allocations: **289,115 allocs / 6.2 MB**
- [x] 2. CPU profile: per-post `tags.collect()` Vec growth (`RawVec::reserve`) + CSR
  walk + sort dominated; 289K Vec allocations
- [x] 3. Optimize: reusable per-post tag buffer + dense tag-count array (contiguous
  Tag range) + size-10 heap — mirrors IC4
- [x] 4. Re-bench: **43 allocs / 352 KB**
- [x] 5. Wall-clock A/B: ~35.3 → ~28.0 ms (~21%), same window
- [x] 6. Value-identity: IC emit byte-identical

## Measurements
| metric                  | baseline   | after |
|-------------------------|------------|-------|
| allocs                  | 289,115    | 43    |
| bytes                   | 6,238,156  | 351,592 |
| wall-clock median (ms)  | ~35.3      | ~28.0 (~21%) |
| hot fn (CPU %)          | per-post Vec collect + sort | CSR walk |

## Notes

IC6 allocated a fresh `Vec` of a post's tags **for every post** (`hasTag(post)
.collect()`) — 289K Vecs / 6.2 MB. Three fixes, mirroring IC4:
- **Reusable tag buffer** (`tags.clear(); tags.extend(...)`) — no per-post Vec.
- **Dense co-occurrence counts** over the contiguous Tag id range instead of a
  growing HashMap (the target tag's slot stays 0 → excluded).
- **Size-10 heap** keyed on (count desc, name asc, borrowed names) instead of
  collect-all-then-sort.

Value-identical. Falls back to the HashMap path if Tag ids are non-contiguous
(still reusing the tag buffer).

**Status: done.**

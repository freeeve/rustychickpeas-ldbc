# 108 — Optimize IC8: recent replies

Per-query optimization loop. Methodology + commands:
[100_ic_perf_methodology.md](100_ic_perf_methodology.md).

## Loop
- [x] 1. Bench allocations: 13 allocs / 333,536 bytes (the `rows` Vec growing to ~13K replies)
- [x] 2. CPU profile: **sort dominated** (`driftsort`/`quicksort` ~most samples)
- [x] 3. Optimize: size-20 heap by (ms desc, id asc) instead of collect-all-then-sort (IC2 shape)
- [x] 4. Re-bench: 1 alloc / 336 bytes
- [x] 5. Wall-clock A/B: ~0.20 → ~0.14 ms, same window
- [x] 6. Value-identity: IC emit byte-identical

## Measurements
| metric                  | baseline | after |
|-------------------------|----------|-------|
| allocs                  | 13       | 1     |
| bytes                   | 333,536  | 336   |
| wall-clock median (ms)  | ~0.20    | ~0.14 |
| hot fn (CPU %)          | sort     | CSR walk |

## Notes

Identical shape to IC2: collected every reply to the seed's messages (~13K for a
high-degree seed → the 326 KB) and sorted to take 20. Replaced with a size-20
heap keyed on (ms, id) — both in hand, no property lookup. The deterministic
allocation drop (326 KB → 336 B) is the headline; wall-clock is small in absolute
terms but the sort was most of it.

**Status: done.**

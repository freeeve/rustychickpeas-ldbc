# 109 — Optimize IC9: recent FoF messages

Per-query optimization loop. Methodology + commands:
[100_ic_perf_methodology.md](100_ic_perf_methodology.md).

## Loop
- [x] 1. Bench allocations: **58 allocs / 154,991,392 bytes (~155 MB)**
- [x] 2. CPU profile: **sort dominated** (`driftsort`/`quicksort`) — collects ~10M
  2-hop FoF messages and sorts the lot
- [x] 3. Optimize: top-20 heap by (ms desc, id asc) + hoist day/ms columns (IC2 + IC3)
- [x] 4. Re-bench: 37 allocs / 287 KB
- [x] 5. Wall-clock A/B: ~177.7 → ~21.4 ms (~8.3×), same window
- [x] 6. Value-identity: IC emit byte-identical

## Measurements
| metric                  | baseline    | after |
|-------------------------|-------------|-------|
| allocs                  | 58          | 37    |
| bytes                   | 154,991,392 | 287,024 |
| wall-clock median (ms)  | ~177.7      | ~21.4 (~8.3×) |
| hot fn (CPU %)          | sort (~10M msgs) | CSR walk + bfs |

## Notes

IC2 at 2-hop scale: it collected **every** FoF message (~10M → 155 MB) and sorted
the lot to take 20, with `pi64("day")`/`pi64("ms")` re-resolved per message.
Replaced with a size-20 heap keyed on (ms, id) plus a hoist of the day/ms columns
(slice/`get`, no per-message key resolution). 8.3× faster, 155 MB → 287 KB,
value-identical. Biggest single-query win of the sweep; IC9 now beats Kùzu (~430 ms)
by ~20× instead of ~2×.

**Status: done.**

# 102 — Optimize IC2: recent friend messages

Per-query optimization loop. Methodology + commands:
[100_ic_perf_methodology.md](100_ic_perf_methodology.md).

## Loop
- [x] 1. Bench allocations (baseline): 19 allocs / 21.8 MB
- [x] 2. CPU profile: sort (`driftsort`/`quicksort`) dominated the sampled subtree
- [x] 3. Optimize: collect-all-then-sort → **bounded top-20 heap** (query-side, no core change)
- [x] 4. Re-bench allocations: 1 alloc / 336 bytes
- [x] 5. Wall-clock A/B: ~18.9 → ~10.6 ms (~1.8×), same window
- [x] 6. Value-identity: IC emit byte-identical

## Measurements
| metric                  | baseline   | after |
|-------------------------|------------|-------|
| allocs                  | 19         | 1     |
| bytes                   | 21,833,152 | 336   |
| wall-clock median (ms)  | ~18.9      | ~10.6 (~1.8×) |
| hot fn (CPU %)          | sort       | CSR walk + pi64 |

## Notes

IC2 collected *every* friend-message (~680K (msg,ms) entries → 21.8 MB) and
sorted the lot to take 20. Replaced with a `BinaryHeap` of size 20 keyed on the
result ordering (ms desc, id asc) stored reversed (min-heap → root is the worst
kept, evicted when a better message arrives). Sort and the giant Vec are gone;
value-identical (id is unique so the top-20 is unambiguous).

Remaining cost is the CSR walk + `pi64(day)`/`pi64(ms)` over ~680K messages.
**Follow-up lever:** hoist the `day`/`ms` column handles once (resolve the key
ids out of the loop, read the column directly) instead of `pi64` re-resolving the
key string per message — same pattern likely helps IC9 and other top-N queries.

**Status: done** (heap optimization). pi64-hoist tracked as a follow-up.

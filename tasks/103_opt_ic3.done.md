# 103 — Optimize IC3: friends in two countries

Per-query optimization loop. Methodology + commands:
[100_ic_perf_methodology.md](100_ic_perf_methodology.md).

## Loop
- [x] 1. Bench allocations (baseline): 44 allocs / 290 KB (not alloc-bound)
- [x] 2. CPU profile: CSR walk `NeighborsByType::next` ~55%; `Atoms::get_id` ~15% (per-msg `pi64("day")`)
- [x] 3. Optimize: hoist the `day` column once (slice/`get`) instead of `pi64` per message
- [x] 4. Re-bench: allocs unchanged (it was a CPU/get_id win)
- [x] 5. Wall-clock A/B: ~130 → ~92 ms (~30%), same window
- [x] 6. Value-identity: IC emit byte-identical

## Measurements
| metric                  | baseline | after |
|-------------------------|----------|-------|
| allocs                  | 44       | 44    |
| bytes                   | 289,968  | 289,968 |
| wall-clock median (ms)  | ~130     | ~92 (~30%) |
| hot fn (CPU %)          | CSR walk + get_id | CSR walk |

## Notes

Hoisted the `day` column (`property_key_from_str` + `as_i64_slice`, BI pattern)
so the inner loop reads `day_s[msg]` directly instead of `pi64(msg,"day")`
re-resolving the `"day"` key (interner lookup) + boxing a `ValueId` for every
friend-of-friend message (~680K). ~30%, value-identical.

**Remaining cost:** the CSR walk, dominated by `neighbors_by_type(msg, "msgCountry")`
per message — which *also* re-resolves the `"msgCountry"` rel-type string each
call. A core `neighbors_by_type_id` (resolve the type once, pass the id) would
cut that and help many IC/BI queries that iterate a constant type in a hot loop.
Tracked as a candidate **core lever** (sign-off) — see report.

**Status: done** (day hoist). msgCountry/by-id-neighbor is a core follow-up.

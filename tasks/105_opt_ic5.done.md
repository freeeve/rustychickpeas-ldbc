# 105 — Optimize IC5: new groups

Per-query optimization loop. Methodology + commands:
[100_ic_perf_methodology.md](100_ic_perf_methodology.md).

## Loop
- [x] 1. Bench allocations: **64,626 allocs / 44.7 MB**
- [x] 2. CPU profile: HashMap/HashSet ops ~biggest (~1340 samples) — a fresh
  `qforums` HashSet allocated *per FoF*; `hd` Column::get; CSR walk
- [x] 3. Optimize: hoist `qforums` out of the FoF loop and `clear()` it (reuse capacity)
- [x] 4. Re-bench: **7,940 allocs / 547 KB**
- [x] 5. Wall-clock A/B: ~270 → ~203 ms (~25%), same window
- [x] 6. Value-identity: IC emit byte-identical

## Measurements
| metric                  | baseline   | after |
|-------------------------|------------|-------|
| allocs                  | 64,626     | 7,940 |
| bytes                   | 44,654,264 | 546,776 |
| wall-clock median (ms)  | ~270       | ~203 (~25%) |
| hot fn (CPU %)          | HashSet alloc + hashing | CSR walk + hashing |

## Notes

`qforums` was `HashSet::new()` *inside* the per-FoF loop — a fresh allocation
(plus growth) for every one of thousands of FoFs (the 44 MB). Hoisted it out and
`clear()` per FoF (clear keeps capacity → one set, reused). Value-identical.

**Remaining 7,940 allocs / 547 KB** (further reducible, tracked):
- `g.relationships(p, Incoming, &["hasMember"])` resolves the `"hasMember"` type
  per FoF → a per-call alloc. Needs a by-id relationships/neighbors variant (**core**).
- `forum_counts` HashMap grows (accumulator) + `qforums` set hashing — candidates
  for a dense generation-stamped scratch over the contiguous forum range
  (**core `scratch_u32`**), which would also kill the hashing CPU.

**Status: done** (qforums reuse). Further alloc cuts depend on the core levers.

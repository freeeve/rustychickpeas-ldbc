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
| metric                  | baseline   | qforums reuse | + RelMatch (core) |
|-------------------------|------------|---------------|-------------------|
| allocs                  | 64,626     | 7,940         | **60** |
| bytes                   | 44,654,264 | 546,776       | **420,696** |
| wall-clock median (ms)  | ~270       | ~203          | **~196** |
| hot fn (CPU %)          | HashSet alloc + hashing | CSR walk + hashing | CSR walk + hashing |

## Notes

`qforums` was `HashSet::new()` *inside* the per-FoF loop — a fresh allocation
(plus growth) for every one of thousands of FoFs (the 44 MB). Hoisted it out and
`clear()` per FoF (clear keeps capacity → one set, reused). Value-identical.

Then the **core `RelMatch` reshape** (rustychickpeas `7979e32`) erased the per-FoF
`Vec`: `g.relationships(p, Incoming, &["hasMember"])` no longer allocates for a
single-type filter (One instead of a 1-element `Set(Vec)`). **7,940 → 60 allocs,
~9% faster** (and value-identical across all IC+BI). Benefits every single-type
slice-filter caller, not just IC5.

**Remaining 60 allocs / 421 KB**: `forum_counts` HashMap growth (accumulator) +
the `reach` map. The last lever is a dense generation-stamped scratch over the
contiguous forum range (**core `scratch_u32`**), which would also kill the
hashing CPU.

**Status: done** (qforums reuse + core RelMatch). `scratch_u32` is the next core lever.

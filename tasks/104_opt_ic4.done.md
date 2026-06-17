# 104 — Optimize IC4: new topics

Per-query optimization loop. Methodology + commands:
[100_ic_perf_methodology.md](100_ic_perf_methodology.md).

## Loop
- [x] 1. Bench allocations: 36 allocs / 322 KB (not alloc-bound)
- [x] 2. CPU profile: no dominant cost — CSR walk ~30%, `get_id`/`pi64("day")` ~15%,
  `in_window` HashMap inserts ~12%, spread evenly
- [x] 3. Decision: **close as already-minimal** (see below)

## Measurements
| metric                  | baseline | after |
|-------------------------|----------|-------|
| allocs                  | 36       | (no change) |
| bytes                   | 321,832  | (no change) |
| wall-clock median (ms)  | ~7.0     | (no change) |
| hot fn (CPU %)          | balanced | — |

## Notes

IC4 is ~7 ms and already wins ~22× vs Kùzu. The only query-side lever is the same
`day`-column hoist used in IC3 (avoid `pi64("day")` per post), worth ~15% ≈ ~1 ms
— below the load-noise floor and pure churn at this size. **Closed without a code
change**, per the methodology's "don't churn already-minimal queries" rule.

Folded into two cross-cutting levers (tracked for a later batched pass):
- **Property-key hoist** (query-side): resolve `day`/`name`/etc. column once
  instead of `pi64`/`pstr` re-resolving the key per element. Applied to IC3
  (30%); marginal here.
- **`neighbors_by_type_id`** (core, sign-off): `neighbors_by_type` re-resolves the
  rel-type string each call; a by-id variant would cut the per-element cost for
  IC3 (`msgCountry`), IC4 (`hasTag`), IC6, … — a broad win to raise with the maintainer.

**Status: done (no change — already minimal).**

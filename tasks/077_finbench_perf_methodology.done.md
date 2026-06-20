# 077 — FinBench complex-read perf methodology

The per-query loop for the 12 FinBench Transaction Complex Reads (TCR1–TCR12),
mirroring [100_ic_perf_methodology.md](100_ic_perf_methodology.md). Each query
gets a task (`079`–`090`) tracking these phases:

1. **Implement** — a faithful Rust traversal in `src/finbench.rs` per the spec
   (LDBC FinBench v0.2.0-alpha §5.1). CR3/CR4/CR11 exist; CR1 partial; the rest new.
2. **Optimize** — bench allocations (deterministic) + CPU profile, optimize, re-bench.
   Same antipatterns as the IC sweep (per-iteration `collect`, sort-truncate vs
   top-K heap, HashMap-vs-dense-array, column/rel-prop hoisting).
3. **Kùzu reference** — Cypher for the query against the Kùzu FinBench DB
   (task `078`), value-cross-checked and timed.
4. **Bench compare** — Rust vs Kùzu wall-clock on SF1 (and SF10 where it matters),
   recorded per query. Timing-only magnitudes labelled until a comparison lands.

## Data
SF1 and SF10 generated via `scripts/gen_finbench.sh` (SF10: `FINBENCH_MEM=24g`).
SF10 = 1.1 M nodes / 9.0 M rels. Seeds chosen per the spec's parameter shape
(high-degree / cycle-bearing accounts), emitted for reproducibility.

## Rel-property capability
The distinctive FinBench shape is **temporal/amount filtering during traversal**:
each `transfer`/`withdraw`/etc. rel carries `ts` + `amt`, read mid-traversal via
the relationship accessor's CSR position (`relationship_property(pos, key)`).

## Choke points
The spec tags each TCR with choke points (truncation on hub vertices, time-window
filtering, recursive path filtering, rel multiplicity). Note the relevant CPs per
query — they're what the optimization pass should target.

## Outcome (done)
All four phases complete for TCR1–TCR12 (tasks 079–090). Results, the
Rust-vs-Kùzu SF10 table, and the before/after optimization numbers are in
[`../docs/finbench-results.md`](../docs/finbench-results.md); raw medians +
allocation profile in `../results/finbench-sf10.txt`. Faithful results are
pinned by `../tests/finbench_queries.rs` (exact-result assertions for all 12).

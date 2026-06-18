# 085 — TCR7: Transfer in/out ratio

**Spec.** LDBC FinBench v0.2.0-alpha §5.1, Transaction Complex Read 7.
From an Account + window, the sum/max and ratio of transfer-in vs transfer-out amounts.

## Phases (see [077](077_finbench_perf_methodology.md))
- [x] Implement (Rust, src/finbench.rs) — new
- [x] Optimize — bench allocations + CPU profile, optimize, re-bench
- [x] Kùzu reference (Cypher; needs [078](078_finbench_kuzu_import.md)) + value cross-check
- [x] Bench compare — Rust vs Kùzu (SF1 / SF10)

**Status: done.** Implement + optimize + Kùzu + bench all complete; see docs/finbench-results.md and tests/finbench_queries.rs.

# 082 — TCR4: Three accounts in a transfer cycle

**Spec.** LDBC FinBench v0.2.0-alpha §5.1, Transaction Complex Read 4.
Detect length-3 transfer cycles (A->B->C->A) through an account with strictly-ascending timestamps and an amount bound.

## Phases (see [077](077_finbench_perf_methodology.md))
- [ ] Implement (Rust, src/finbench.rs) — DONE (representative): transfer_cycles (general len<=6); specialize/verify the 3-account spec
- [x] Optimize — bench allocations + CPU profile, optimize, re-bench
- [x] Kùzu reference (Cypher; needs [078](078_finbench_kuzu_import.md)) + value cross-check
- [x] Bench compare — Rust vs Kùzu (SF1 / SF10)

**Status: done.** Implement + optimize + Kùzu + bench all complete; see docs/finbench-results.md and tests/finbench_queries.rs.

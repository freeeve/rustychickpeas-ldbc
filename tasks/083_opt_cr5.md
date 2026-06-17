# 083 — TCR5: Exact Account Transfer Trace

**Spec.** LDBC FinBench v0.2.0-alpha §5.1, Transaction Complex Read 5.
From a Person + window, the exact downstream transfer trace from the person's owned accounts.

## Phases (see [077](077_finbench_perf_methodology.md))
- [x] Implement (Rust, src/finbench.rs) — new
- [ ] Optimize — bench allocations + CPU profile, optimize, re-bench
- [x] Kùzu reference (Cypher; needs [078](078_finbench_kuzu_import.md)) + value cross-check
- [x] Bench compare — Rust vs Kùzu (SF1 / SF10)

**Status: pending.**

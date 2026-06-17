# 080 — TCR2: Fund gathered from the accounts applying loans

**Spec.** LDBC FinBench v0.2.0-alpha §5.1, Transaction Complex Read 2.
From a Person + window, find an account owned by the person and the fund gathered into it from accounts that applied for loans.

## Phases (see [077](077_finbench_perf_methodology.md))
- [x] Implement (Rust, src/finbench.rs) — new
- [ ] Optimize — bench allocations + CPU profile, optimize, re-bench
- [x] Kùzu reference (Cypher; needs [078](078_finbench_kuzu_import.md)) + value cross-check
- [x] Bench compare — Rust vs Kùzu (SF1 / SF10)

**Status: pending.**

# 086 — TCR8: Transfer trace after loan applied

**Spec.** LDBC FinBench v0.2.0-alpha §5.1, Transaction Complex Read 8.
From a Loan + window, trace the fund flow from the loan (loan -> deposit -> account -> transfers).

## Phases (see [077](077_finbench_perf_methodology.md))
- [x] Implement (Rust, src/finbench.rs) — new
- [ ] Optimize — bench allocations + CPU profile, optimize, re-bench
- [x] Kùzu reference (Cypher; needs [078](078_finbench_kuzu_import.md)) + value cross-check
- [ ] Bench compare — Rust vs Kùzu (SF1 / SF10)

**Status: pending.**

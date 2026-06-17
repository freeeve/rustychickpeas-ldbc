# 087 — TCR9: Money laundering with loan involved

**Spec.** LDBC FinBench v0.2.0-alpha §5.1, Transaction Complex Read 9.
From an account + amount bound + window, detect laundering paths involving a loan (deposit/repay + transfer cycle).

## Phases (see [077](077_finbench_perf_methodology.md))
- [x] Implement (Rust, src/finbench.rs) — new
- [ ] Optimize — bench allocations + CPU profile, optimize, re-bench
- [x] Kùzu reference (Cypher; needs [078](078_finbench_kuzu_import.md)) + value cross-check
- [ ] Bench compare — Rust vs Kùzu (SF1 / SF10)

**Status: pending.**

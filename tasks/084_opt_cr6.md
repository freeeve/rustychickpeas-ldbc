# 084 — TCR6: Withdrawal after Many-to-One transfer

**Spec.** LDBC FinBench v0.2.0-alpha §5.1, Transaction Complex Read 6.
From a card Account + window, many-to-one transfer-ins followed by a withdrawal.

## Phases (see [077](077_finbench_perf_methodology.md))
- [ ] Implement (Rust, src/finbench.rs) — new
- [ ] Optimize — bench allocations + CPU profile, optimize, re-bench
- [ ] Kùzu reference (Cypher; needs [078](078_finbench_kuzu_import.md)) + value cross-check
- [ ] Bench compare — Rust vs Kùzu (SF1 / SF10)

**Status: pending.**

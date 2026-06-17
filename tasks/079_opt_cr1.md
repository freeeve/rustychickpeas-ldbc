# 079 — TCR1: Blocked medium related accounts

**Spec.** LDBC FinBench v0.2.0-alpha §5.1, Transaction Complex Read 1.
From an Account + time window, find Accounts signed in by a blocked Medium reachable by a <=3-step time-ascending transfer trace; return account id, distance, medium id/type.

## Phases (see [077](077_finbench_perf_methodology.md))
- [x] Implement (Rust, src/finbench.rs) — PARTIAL: trace_transfers_in has the windowed reverse BFS — add the blocked-Medium signIn filter + distance/medium output
- [ ] Optimize — bench allocations + CPU profile, optimize, re-bench
- [ ] Kùzu reference (Cypher; needs [078](078_finbench_kuzu_import.md)) + value cross-check
- [ ] Bench compare — Rust vs Kùzu (SF1 / SF10)

**Status: pending.**

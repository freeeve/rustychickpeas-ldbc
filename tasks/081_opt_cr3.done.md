# 081 — TCR3: Shortest transfer path

**Spec.** LDBC FinBench v0.2.0-alpha §5.1, Transaction Complex Read 3.
Shortest transfer path (hop count) between two accounts within a time window.

## Phases (see [077](077_finbench_perf_methodology.md))
- [ ] Implement (Rust, src/finbench.rs) — DONE (representative): shortest_transfer_path; verify spec params (truncation on hubs)
- [x] Optimize — bench allocations + CPU profile, optimize, re-bench
- [x] Kùzu reference (Cypher; needs [078](078_finbench_kuzu_import.md)) + value cross-check
- [x] Bench compare — Rust vs Kùzu (SF1 / SF10)

**Status: done.** Implement + optimize + Kùzu + bench all complete; see docs/finbench-results.md and tests/finbench_queries.rs.

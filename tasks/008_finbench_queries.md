# 008 — FinBench transaction-tracing queries

**Goal.** Implement a representative subset of FinBench complex reads as hand-
coded temporal traversals, and time them.

**Why.** Transaction tracing (temporal fund-flow paths, transfer cycles, blocked-
account propagation) is the traversal-with-edge-properties shape Q11 already
drove — a strong fit for the CSR/relationship-accessor engine.

**Depends on.** 007. Stubs in `src/finbench.rs`.

**Scope — representative complex reads:**
- trace inbound `transfer` paths into an account within a time window
  (`trace_transfers_in`)
- time-ordered transfer cycles above an amount threshold (`transfer_cycles`)
- shortest in-window `transfer` path between two accounts
  (`shortest_transfer_path`)
- guarantee-chain loan exposure (`guarantee_exposure`)

**Steps.**
1. Fill the `finbench` stubs; the time-window/amount filters read edge properties
   during traversal via the relationship accessor (reuse the Q11 pattern).
2. Pick reproducible seed accounts (highest transfer degree), emit them.
3. Smoke-test result shapes; time with `time_query`.
4. Optional reference side: Kùzu or TuGraph FinBench impl for a head-to-head, if
   cheap; otherwise timing-only and labelled as such.

**Acceptance.**
- The four reads run on the smallest SF with stable, non-empty results.
- Per-query timings printed; any unverified magnitude labelled (no published
  comparison implied without one).

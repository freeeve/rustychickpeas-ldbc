# 008 — FinBench transaction-tracing queries

**Goal.** Implement a representative subset of FinBench complex reads as hand-coded
temporal traversals, and time them.

## Done

Four temporal traversals in `src/finbench.rs`, each reading per-rel `ts`/`amt`
mid-traversal via the relationship accessor's CSR position
(`relationship_property(pos, key)`):

- **`trace_transfers_in`** — reverse BFS over incoming `transfer` rels, ≤k hops,
  keeping only rels whose timestamp is in `[start, end]`; returns upstream accounts.
- **`transfer_cycles`** — DFS for fund-transfer cycles back to the seed with
  strictly-increasing time, amount ≥ threshold, completing within a window; bounded
  by length (6) and a 1000-cycle cap.
- **`shortest_transfer_path`** — `weighted_shortest_path` over `transfer` with the
  Dijkstra weight closure pruning out-of-window rels (unit weight → hop count).
- **`guarantee_exposure`** — BFS the `guarantee` chain, summing the `apply` (loan)
  amounts the person and everyone they guarantee are on the hook for.

`src/bin/finbench.rs` seeds by highest transfer degree (and scans the top accounts
for one actually on a cycle, since cycles are sparse), then runs + times all four
with `harness::time_query`.

## Acceptance — met (SF1, timing-only — no published comparison implied)
```
seeds: account=29755 (deg 948), cycle-account=7923, dst=31526, person=53919
  trace_transfers_in(<=3 hops): 6 upstream accounts
  transfer_cycles(>=1000, 90d): 1 cycles
  shortest_transfer_path(29755->31526): 3 hops
  guarantee_exposure(person 53919): 122133184.00
FB1 trace_transfers_in        0.00 ms
FB2 transfer_cycles           0.00 ms
FB3 shortest_transfer_path    0.05 ms
FB4 guarantee_exposure        0.00 ms
```
All four run with stable, non-empty results; per-query timings printed. No
reference engine wired (step 4 optional) — timings are labelled timing-only.

**Status: done.**

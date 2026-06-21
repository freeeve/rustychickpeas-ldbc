# FinBench — LDBC FinBench Transaction Complex Reads (SF10)

[← benchmark hub](../README.md) · prior detailed head-to-head + methodology:
[finbench-results.md](finbench-results.md)

The 12 Transaction Complex Reads (`TCR1`–`TCR12`) of LDBC FinBench v0.2.0-alpha — a
transaction-network schema (Account / `transfer` / `withdraw` / loan) with
fraud-tracing-shape temporal-path and fund-cycle queries. These lean on the
rel-`creationDate`-during-traversal capability BI Q11 drove into core.

## Scale

FinBench **SF10**: **1,103,805 nodes / 8,962,710 rels**, loaded from the raw generator
output in ~13.5 s.

> **Conditions.** Apple M3 Max, `target/release/finbench`, median of 5, ~3–4 cores of
> background load. The TCRs are seed-anchored point queries — bounded time-ordered
> transfer traces from a single account — so they're sub-millisecond; `result` is the
> row/aggregate count.

## rustychickpeas — TCR1–TCR12, SF10

| Query | Time | Result | | Query | Time | Result |
|-------|-----:|--------|-|-------|-----:|--------|
| CR1 blocked-medium | 0.98 ms | 605 | | CR7 in-out-ratio | 0.41 ms | 3 |
| CR2 loan-gather | <0.01 ms | 2 | | CR8 loan-fund-trace | 0.03 ms | 124 |
| CR3 shortest-path | 0.73 ms | 2 | | CR9 laundering | 0.29 ms | 0 |
| CR4 3-cycle | <0.01 ms | 1 | | CR10 investor-sim | <0.01 ms | 56 |
| CR5 downstream-trace | <0.01 ms | 4 | | CR11 guarantee-chain | <0.01 ms | 302,858,089 |
| CR6 withdraw-after-in | <0.01 ms | 1 | | CR12 company-transfer | <0.01 ms | 0 |

## Kùzu head-to-head

Re-benched on SF10: `.venv-kuzu/bin/python kuzu/finbench_queries.py kuzu/db-finbench-sf10`
(Kùzu 0.11.3, median of 7). Both sides pick the highest-degree seeds the same way, so
every TCR is comparable; the Kùzu driver inlines window bounds as literals and unrolls
CR2's reverse-reachability into explicit hops (its recursive engine explodes on hub
accounts — see the harness notes).

| Query | rustychickpeas | Kùzu | winner |
|-------|---------------:|-----:|--------|
| CR1 blocked-medium | 0.98 ms | 5.81 ms | rustychickpeas |
| CR2 loan-gather | <0.01 ms | 29.62 ms | rustychickpeas |
| CR3 shortest-path | 0.73 ms | 5.87 ms | rustychickpeas |
| CR4 3-cycle | <0.01 ms | 419.39 ms | rustychickpeas |
| CR5 downstream-trace | <0.01 ms | 2.28 ms | rustychickpeas |
| CR6 withdraw-after-in | <0.01 ms | 1.49 ms | rustychickpeas |
| CR7 in-out-ratio | 0.41 ms | 3.81 ms | rustychickpeas |
| CR8 loan-fund-trace | 0.03 ms | 2.17 ms | rustychickpeas |
| CR9 laundering | 0.29 ms | 36.37 ms | rustychickpeas |
| CR10 investor-sim | <0.01 ms | 6.42 ms | rustychickpeas |
| CR11 guarantee-chain | <0.01 ms | 3.03 ms | rustychickpeas |
| CR12 company-transfer | <0.01 ms | 18.46 ms | rustychickpeas |

The TCRs are seed-anchored point queries — bounded time-ordered traces from a single
account — exactly the transactional shape the CSR adjacency is built for, so
rustychickpeas wins every row. CR4 (the 3-cycle) is the widest gap: Kùzu materializes the
3-hop cycle join (419 ms) where our traversal short-circuits on the time order.

> **Honesty caveat.** Kùzu is multi-threaded with an optimizer; our queries are
> single-threaded traversals. Both runs are on the same Apple M3 Max with ~3–4 cores of
> background load, and seeds/windows differ slightly (so result counts differ) — read the
> times as order-of-magnitude. The durable result is the value-for-value cross-check
> below.

## Validation

All 12 TCRs are implemented and cross-checked value-for-value against Kùzu on SF10 (see
[finbench-results.md](finbench-results.md) and `tests/finbench_queries.rs`).

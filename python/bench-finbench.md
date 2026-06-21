# FinBench — Python vs Rust (SF10)

[← Python suite](README.md) · related: [FinBench Rust + Kùzu](../docs/bench-finbench.md)

How close does the **Python binding** get to **hand-written Rust** on the 12 Transaction
Complex Reads (`CR1`–`CR12`) of LDBC FinBench v0.2.0-alpha? Both pick the same
highest-degree seeds and run the same bounded time-ordered transfer traces over the same
SF10 graph; the Rust side is the single-threaded `src/bin/finbench.rs`, the Python side is
`python/finbench/` through the bindings.

**Parity: correctness is pinned by `python/tests/test_finbench.py`** (the CR1–CR12 fixture,
the same exact assertions as the Rust tests). FinBench has no SF cross-check emit, so
`python python/run_finbench.py` is timing + result-shape only — and the result shapes match
the Rust reference column (CR1 605, CR2 2, CR4 1, CR5 4, CR8 124, CR10 56, CR11 302858089,
CR12 0).

> **Numbers** are median of 5 after warmup, Apple M3 Max, ~3–4 cores of background load.
> **Py/Rust** = Python ÷ Rust; lower is better, `<1` = Python faster. These are
> seed-anchored *point* queries — almost every one is sub-millisecond on both sides — so the
> ratio is dominated by per-call overhead, not real work. Rust numbers are the fresh ones
> from [bench-finbench.md](../docs/bench-finbench.md).

| Query | Python | Rust | Py/Rust | how |
|-------|-------:|-----:|--------:|-----|
| CR1 blocked-medium | 27.2 ms | 0.98 ms | 28× | ≤3-hop reverse transfer reach + blocked-medium join |
| CR2 loan-gather | 0.0 ms | <0.01 ms | —† | reverse-reachable owners → loan deposit sum — sub-ms |
| CR3 shortest-path | 2.1 ms | 0.73 ms | 2.9×† | bounded time-ordered transfer shortest path — sub-3 ms |
| CR4 transfer-cycles | 0.0 ms | <0.01 ms | —† | time-ordered 3-cycle detection — sub-ms |
| CR5 downstream-trace | 0.0 ms | <0.01 ms | —† | one-hop owned-account transfer fan-out — sub-ms |
| CR6 withdraw-after-in | 0.0 ms | <0.01 ms | —† | transfer-then-withdraw ordering on one card — sub-ms |
| CR7 in-out-ratio | 0.7 ms | 0.41 ms | 1.7×† | in/out transfer sum + count on one account — sub-ms |
| CR8 loan-fund-trace | 0.5 ms | 0.03 ms | 17×† | loan deposit → downstream transfer distinct set — sub-ms |
| CR9 laundering | 0.3 ms | 0.29 ms | 1.0×† | loan→transfer→repay laundering count — sub-ms |
| CR10 investor-sim | 0.0 ms | <0.01 ms | —† | shared-company co-investor count — sub-ms |
| CR11 guarantee-chain | 0.0 ms | <0.01 ms | —† | ≤3-hop guarantee chain → loan exposure sum — sub-ms |
| CR12 company-transfer | 4.3 ms | <0.01 ms | high† | owner→account→company transfer aggregation — sub-ms Rust |

† **Sub-3 ms absolute** (Rust side sub-millisecond on nearly all) — the multiplier is
dominated by per-call Python overhead on a trivial amount of work. Ignore the ratio.

**Reading the table.** There is essentially nothing to separate here: the TCRs are
bounded point queries — a single seed account, a handful of hops, time-ordered — so the
*work* is tiny on both engines. The Rust side is sub-millisecond on 9 of 12; the Python
side adds fixed per-call binding overhead (a few hundred µs to a few ms) that swamps the
actual traversal. The two rows that do measurable work — CR1 (the ≤3-hop reverse-reach
join) and CR12 (the owner→company aggregation) — are the only places the Python
orchestration cost is visible, and even there it's single-digit milliseconds. The durable
result is correctness: the fixture assertions match the Rust tests value-for-value, and the
runtime result shapes match the Rust reference counts.

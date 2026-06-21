# Interactive (IC/IS) — Python vs Rust (SF1)

[← Python suite](README.md) · related: [IC Rust + Kùzu](../docs/bench-interactive.md)

How close does the **Python binding** get to **hand-written Rust** on the 14 complex
reads (`IC1`–`IC14`) and the short reads (`IS1`–`IS7`)? Both run the same seed-anchored
query over the same SF1 graph; the Rust side is the single-threaded `src/bin/ic.rs`, the
Python side is `python/ic/` through the bindings, driving native CSR traversal /
BFS / Dijkstra primitives.

**Parity: 20/20 queries match the Rust reference** (`python python/run_ic.py`, diffed
against `python/refs/*.rust.json`).

> **Numbers** are median of 5 after warmup, Apple M3 Max, ~3–4 cores of background load.
> **Py/Rust** = Python ÷ Rust; lower is better, `<1` = Python faster. The ratio (same
> data, same machine, back-to-back) is the robust signal — read it, not the milliseconds.
> Rust numbers are the fresh ones from [bench-interactive.md](../docs/bench-interactive.md).

| Query | Python | Rust | Py/Rust | how |
|-------|-------:|-----:|--------:|-----|
| IC1 friends-by-name | 17.4 ms | 13.8 ms | 1.3× | native bounded-`knows` BFS distance anchoring |
| IC2 recent friend messages | 151.4 ms | 28.7 ms | 5.3× | friend message scan + recency top-k |
| IC3 two countries | 982.6 ms | 169.7 ms | 5.8× | broad FoF neighbourhood + per-message country grouping |
| IC4 new topics | 155.9 ms | 4.3 ms | 36× | friends' post tags with a never-before set-difference |
| IC5 new groups | 2650.5 ms | 349.4 ms | 7.6× | widest FoF walk + group-membership scan |
| IC6 tag co-occurrence | 911.4 ms | 46.8 ms | 19× | 2-hop neighbourhood + co-tag counting |
| IC7 recent likers | 9.2 ms | 0.7 ms | 13× | likers of own messages + latest-like reduce |
| IC8 recent replies | 3.4 ms | 0.3 ms | 11× | direct reply iteration — small result |
| IC9 recent FoF messages | 1134.4 ms | 41.0 ms | 28× | ≤2-hop friend set + recency top-k over their messages |
| IC10 friend recommend | 152.7 ms | 5.7 ms | 27× | exact-2-hop foaf + interest-overlap scoring |
| IC11 job referral | 24.0 ms | 5.9 ms | 4.1× | neighbourhood `workAt` filter by company place |
| IC12 expert search | 482.4 ms | 62.6 ms | 7.7× | friends' replies under a tag-class subtree |
| IC13 unweighted shortest path | 9.7 ms | 1.2 ms | 8.1× | native unweighted `knows` BFS path |
| IC14 weighted shortest path | 35.5 ms | 17.6 ms | 2.0× | native single-source Dijkstra over interaction weights |
| IS1 person profile | 0.0 ms | <0.01 ms | —†  | single node-property fetch — sub-ms |
| IS2 person recent messages | 7.4 ms | 0.19 ms | 39×† | own-message scan + recency top-10 |
| IS3 person friends | 0.1 ms | 0.01 ms | 10×† | direct CSR neighbour iteration — sub-ms |
| IS5 message creator | 6.6 ms | — | — | creator of most-recent message (no published Rust IS5 timing) |
| IS6 forum of message | 0.0 ms | <0.01 ms | —† | one container-of hop — sub-ms |
| IS7 replies of message | 0.1 ms | 0.04 ms | 2.5×† | direct reply iteration + knows flag — sub-ms |

† **Sub-3 ms absolute** — the multiplier (where shown) is dominated by per-call Python
overhead on a trivial amount of work, not by the query doing real work slowly. Ignore the
ratio.

**Reading the table.** Strip the sub-3 ms short reads (†, where the ratio is noise) and
the complex reads fall into three groups:

- **At parity / near it (≤ 2×)** — IC1, IC14. These are dominated by a single native
  primitive (bounded BFS, single-source Dijkstra) with the GIL released; Python just
  drives the call, so it tracks Rust closely.
- **Native win, residual gap (4–8×)** — IC2, IC3, IC5, IC11, IC12, IC13. A native
  traversal carries the hot loop, but the orchestration (grouping, set ops, recency
  ordering) stays in Python and shows up as a single-digit multiple.
- **Python-orchestration-bound (13–36×)** — IC4, IC6, IC7, IC9, IC10. These do
  set-difference / co-occurrence / interest-overlap bookkeeping over wide
  neighbourhoods, where the per-element work lives in the interpreter rather than a
  bulk kernel. They are the candidates for a future native primitive, the same way BI's
  `where_via` closed Q12.

The headline: the seed-anchored CSR shape is where the bindings are weakest *relative* to
Rust — the heavy complex reads lean on Python-side bookkeeping that the BI pass moved into
native kernels but the IC tier hasn't yet — yet every query still validates 20/20, and the
two pure shortest-path queries (IC13, IC14) land within single-digit and 2× of hand-coded
Rust because their hot loop is already a native primitive.

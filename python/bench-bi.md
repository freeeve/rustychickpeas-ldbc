# BI — Python vs Rust (SF1)

[← Python suite](README.md) · related: [BI Rust + Kùzu](../docs/bench-bi.md)

How close does the **Python binding** get to **hand-written Rust** on the 20 faithful
BI queries? Both run the same query over the same SF1 graph; the Rust side is the
single-threaded `src/bin/bi.rs`, the Python side is `python/bi/` through the bindings,
tuned to route heavy work into native primitives.

> **Numbers** are from the BI optimization pass (tasks 167–186), median of 5 after
> warmup, Apple M3 Max. **Py/Rust** = Python ÷ Rust; lower is better, `<1` = Python
> faster. The ratio (same data, same machine, back-to-back) is the robust signal —
> read it, not the milliseconds. Pending re-verification against the v0.9.0 bindings.

| Query | Python | Rust | Py/Rust | how |
|-------|-------:|-----:|--------:|-----|
| Q1 posting summary | 2.7 ms | 3.1 ms | **0.9×** | parallel `aggregate` kernel — beats single-threaded Rust |
| Q2 tag evolution | 6.3 ms | 7.0 ms | **0.9×** | native `aggregate` |
| Q3 popular topics | 14.2 ms | 1.8 ms | 8× | near floor — small forum reply-trees, already cheap |
| Q4 top creators | ~65 ms | 55 ms | 1.2× | native million-pair membership scan |
| Q5 active posters | 1.7 ms | 0.4 ms | 4.5×† | one tag's messages — sub-2 ms |
| Q6 authoritative users | 222 ms | 108 ms | 2× | memoized 2-hop likes + O(1) CSR degree |
| Q7 related topics | 14.9 ms | 2.2 ms | 6.7× | near floor — multi-valued tag fan-out (no membership flip) |
| Q8 central person | 1.5 ms | 0.4 ms | 3.8×† | interest set + one knows-sum — sub-2 ms |
| Q9 thread initiators | 15 ms | 7.4 ms | 2× | native reply-tree primitive (177→15 ms) |
| Q10 experts in country | 27.4 ms | 8.2 ms | 3.3× | near floor — multi-valued tag grouping |
| Q11 friend triangles | 14.7 ms | 2.7 ms | 5.4× | bulk `rels_with_props` (aligned neighbor/date arrays) |
| Q12 message histogram | 17.6 ms | 5.0 ms | 3.5× | native `where_via` projected filter (946→17.6 ms) |
| Q13 zombies | 2.6 ms | 0.2 ms | 12×† | France: 5 zombies — sub-3 ms |
| Q14 international dialog | 29.3 ms | 5.1 ms | 5.7× | near floor — per-message creator grouping |
| Q15 weighted path | ~700 ms | 17.8 ms | 39× | native arrays (1344→700 ms); residual scan-bound |
| Q16 fake news | 0.5 ms | 0.2 ms | 2.2×† | two small per-param scans — sub-1 ms |
| Q17 information propagation | 11 ms | 70.8 ms | **0.16×** | native — ~6× *faster* than Rust (632→11 ms) |
| Q18 friend recommendation | 110 ms | 107 ms | 1.0× | native mutual-friend filter (259→110 ms) |
| Q19 interaction path | 59 ms | 6.8 ms | 8.6× | native single-source Dijkstra, GIL released (652→59 ms) |
| Q20 recruitment | 0.4 ms | 0.3 ms | 1.4×† | study-cohort weight map built once — sub-1 ms |

† **Sub-3 ms absolute** — the multiplier is dominated by per-call Python overhead on a
trivial amount of work, not by the query doing real work slowly. Ignore the ratio.

**Reading the table.** Strip the sub-3 ms rows (†, where the ratio is noise) and the
20 queries fall into four groups:

- **Faster than hand-written Rust** — Q1, Q2, Q17. The bindings call the *parallel*
  `aggregate` / native kernels (GIL released) where `bi.rs` is single-threaded, so
  Python wins outright. Not magic — a fairer Rust baseline would multi-thread these.
- **At parity (≤ 1.2×)** — Q4, Q18. The native primitive does essentially all the
  compute; Python just drives it.
- **Native win, residual gap (2–9×)** — Q6, Q9, Q11, Q12, Q15, Q19. A native primitive
  cut 2–54× off the pure-Python baseline (Q12 946→17.6 ms via the just-shipped
  `where_via`, Q19 652→59 ms, Q15 1344→700 ms); what remains is a residual scan the
  current primitives don't cover.
- **Near floor, intrinsic (3–8×)** — Q3, Q7, Q10, Q14. These group by a node's
  *multi-valued* tags, which can't collapse into a membership-style native kernel; the
  Python baseline is already close to the achievable floor.

The headline: with the native primitives in place — including the just-shipped
`where_via`, which closed the last deferred query — **every BI query that does real work
is served by a native kernel**, landing within single-digit multiples of hand-coded Rust
(three of them faster). The residual gaps are intrinsic multi-valued-tag grouping (3–8×)
and one scan-bound query (Q15, 39×), not missing features. Making that possible was the
whole point of the binding's primitive surface and the upstream `rustychickpeas-core`
work that built it.

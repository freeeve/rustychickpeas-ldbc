# SPB — Python vs Rust (Semantic Publishing Benchmark)

[← Python suite](README.md) · related: [SPB Rust + Oxigraph](../docs/bench-spb.md)

How close does the **Python binding** get to **hand-written Rust** on the SPB query set
(`q1`–`q9`, `a1`–`a25`)? SPB is RDF/SPARQL natively; both sides parse the same N-Triples
extract into a property graph and run the hand-translated traversals — no triple store, no
reasoner. The Rust side is `src/bin/spb_parity`, the Python side is `python/spb/` through
the bindings (the full-text / geo families run Python replicas of the core inverted index
/ bbox scan, since the binding does not expose `full_text_search` / geo).

**Parity: 30/30 queries match the Rust parity reference** (`python python/run_spb.py`,
diffed value-for-value against `results/spb.parity.rust.json` — the same reference Oxigraph
validated 30/30).

> **Numbers** are median of 3 (`SPB_RUNS`) after warmup, Apple M3 Max, ~3–4 cores of
> background load. **Py/Rust** = Python ÷ Rust; lower is better, `<1` = Python faster.
> Rust numbers are the fresh ones from [bench-spb.md](../docs/bench-spb.md) (median of 5).
> Read the ratio, not the milliseconds.

| Query | Python | Rust | Py/Rust | how |
|-------|-------:|-----:|--------:|-----|
| q1 minute histogram | 12.0 ms | 1.0 ms | 12× | timestamp grouping scan |
| q2 single creative-work | 0.0 ms | <0.01 ms | —† | one-resource lookup — sub-ms |
| q3 popular topics | 9.2 ms | 1.0 ms | 9× | topic fan-out scan |
| q4 about-topic | 5.4 ms | 0.67 ms | 8× | about-relation scan |
| q5 date window | 92.1 ms | 5.8 ms | 16× | date-range filter + type/audience join |
| q7 category window | 19.4 ms | 1.8 ms | 11× | category + audience window filter |
| q9 fulltext union | 60.1 ms | 3.8 ms | 16× | multi-creative-work union scan |
| a1 about-entity | 8.7 ms | 1.0 ms | 9× | about-relation entity scan |
| a2 entity rows | 0.0 ms | <0.01 ms | —† | one-resource lookup — sub-ms |
| a3 date count | 13.4 ms | 1.5 ms | 9× | date-range aggregate |
| a4 date list | 12.0 ms | 1.4 ms | 9× | date-range list |
| a5 about-entity (large) | 275.3 ms | 20.7 ms | 13× | 108 K-row entity-type restricted scan |
| a6 audience rows | 33.3 ms | 4.2 ms | 8× | audience filter scan |
| a7 type scan | 44.9 ms | 4.2 ms | 11× | full type scan (33 K rows) |
| a8 type+audience window | 23.0 ms | 1.9 ms | 12× | type + audience + date window |
| a9 max aggregate | 8.1 ms | 1.3 ms | 6× | single max reduce |
| a10 small aggregate | 13.3 ms | 1.4 ms | 10× | grouped aggregate (16 rows) |
| a13 tag pairs | 644.5 ms | 44.6 ms | 14× | 336 K-row tag co-occurrence pairs |
| a14 format/doc filter | 67.8 ms | 7.4 ms | 9× | primary-format + web-doc-type scan |
| a15 fulltext word | 0.0 ms | <0.01 ms | —† | inverted-index lookup (Python replica) — sub-ms |
| a16 fulltext rows | 0.1 ms | 0.03 ms | 4×† | inverted-index lookup — sub-ms |
| a17 geo bbox | 2.2 ms | 0.65 ms | 3×† | bbox/KD scan (Python replica) — sub-3 ms |
| a18 type date window | 9.0 ms | 1.0 ms | 9× | type + date-range scan |
| a19 type+audience window | 109.8 ms | 8.4 ms | 13× | 11 K-row type/audience window |
| a20 fulltext word | 0.4 ms | 0.02 ms | 17×† | inverted-index lookup — sub-ms |
| a21 fulltext + facets | 1.0 ms | 0.03 ms | 31×† | inverted index + category/audience facets — sub-ms |
| a22 fulltext + date | 0.9 ms | 0.09 ms | 11×† | inverted index + date window — sub-ms |
| a23 fulltext + category | 6.7 ms | 0.61 ms | 11× | inverted index + category aggregate |
| a24 relatedness pair | 0.9 ms | 0.12 ms | 8×† | two-entity relatedness — sub-ms |
| a25 relatedness | 97.0 ms | 7.2 ms | 14× | 47 K-row topic relatedness scan |

† **Sub-3 ms absolute** — the multiplier is dominated by per-call Python overhead on a
trivial amount of work. Ignore the ratio.

**Reading the table.** SPB is the scan-heavy family — most queries are a full or
range-restricted property scan with a grouped aggregate, which is exactly the shape that
stays in the Python interpreter rather than collapsing into a single bulk kernel. So the
real-work queries cluster at **13–16×** (q5, q9, a5, a13, a19, a25 — the large-result
scans), a tight, consistent band: the Python side pays a per-row orchestration cost that
Rust does inline. The full-text and geo queries (a15–a17, a20–a22) are sub-3 ms on both
sides because the inverted-index / bbox lookup is small and indexed — but note the Python
side runs a *replica* of the core index there, since the binding does not yet expose
`full_text_search` / geo; exposing those primitives is the obvious next step to pull this
family toward the BI-style single-digit floor. The durable result is unchanged: **30/30
value-identical** against the Oxigraph-validated reference.

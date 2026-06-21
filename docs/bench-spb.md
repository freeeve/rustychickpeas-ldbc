# SPB — Semantic Publishing Benchmark

[← benchmark hub](../README.md) · related: [SPB Python vs Rust](../python/bench-spb.md)

SPB is RDF/SPARQL natively; we parse the N-Triples serialization, map it to a property
graph (IRI object → rel, literal → property, `rdf:type` → label), and hand-translate the
SPARQL templates into Rust traversals — **no triple store, no reasoner**. The full-text
and geo queries run off two core indexes (an inverted index + a KD-tree) that this family
drove into `rustychickpeas-core`.

```bash
cargo run --release --bin spb_parity
```

> **Re-bench pending.** The timings below are from the prior pass, and the Rust SPB suite
> is currently mid-refactor (uncommitted) — they'll be refreshed once that lands. The
> **30/30 parity vs Oxigraph** validation is the durable result.

## Scale & validation — 30/30 value-identical vs Oxigraph

`scripts/spb_parity.py` runs the original SPARQL against a local
[Oxigraph](https://github.com/oxigraph/oxigraph) store over the *same* 3.85 M-triple
extract and diffs row-for-row against our results — every query (q1–q9, a1–a25)
**MATCHES**. This is the strongest correctness signal in the suite: value-identity against
an independent SPARQL engine, not a shape check.

Indicative timings (single run, M3 Max):

| Query | Time | Rows | | Query | Time | Rows |
|-------|-----:|-----:|-|-------|-----:|-----:|
| q1 minute histogram | 1.0 ms | 9457 | | a5 about-entity | 21 ms | 108476 |
| q9 fulltext union | 4.5 ms | 9462 | | a13 tag pairs | 40 ms | 336315 |
| q5 date window | 5.9 ms | 7898 | | a25 relatedness | 8.6 ms | 47499 |

(full 30-query table: the parity-script output + `results/spb.parity.rust.json`.)

## What this family drove into core

SPB's aggregation subset is scan-heavy (the shape we lose to columnar engines) — coverage
breadth, not a head-to-head win. But its **full-text and geo queries drove two genuinely
new core capabilities** into `rustychickpeas-core`:

- an **inverted index** (`full_text_search` / `full_text_search_ranked`), and
- a **geo-spatial KD-tree**,

both returning `NodeSet` so they compose with label sets and traversal — the same
"a benchmark surfaces a missing capability we fix upstream" story as the relationship
accessor and `dijkstra`.

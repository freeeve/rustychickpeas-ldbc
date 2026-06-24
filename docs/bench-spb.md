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

The **30/30 parity vs Oxigraph** validation is the durable result; the per-query timings
below are a fresh `spb_parity` run (median of 5).

## Scale & validation — 30/30 value-identical vs Oxigraph

`scripts/spb_parity.py` runs **hand-adapted** SPARQL (mirroring our hand-translation's
modeling — *not* the verbatim LDBC SPB templates) against a local
[Oxigraph](https://github.com/oxigraph/oxigraph) store over the *same* 3.85 M-triple
extract and diffs row-for-row — **30/30 MATCH**. Oxigraph independently recomputes every
join / GROUP BY / aggregation with the same parameters, so this is a strong, params-aligned
second-engine check of the **query mechanics**. Two honest limits: because the SPARQL was
co-designed to match our modeling, *shared* modeling choices (e.g. `tag → about ∪ mentions`,
and a whole-word full-text approximation of SPB's true `CONTAINS`/Lucene) aren't
independently validated; and the two signature core-driver queries — **q6 (geo)** and
**q8 (full-text)** — are *not* in the Oxigraph 30, but cross-checked by a separate Python
reference (`scripts/spb_crosscheck.py`). The 30/30 is recorded in the run output but is
**not yet reproducible from committed artifacts** (the Oxigraph store + result dumps are
gitignored).

Indicative timings (median of 5, M3 Max):

| Query | Time | Rows | | Query | Time | Rows |
|-------|-----:|-----:|-|-------|-----:|-----:|
| q1 minute histogram | 1.0 ms | 9457 | | a5 about-entity | 21 ms | 108476 |
| q9 shared-tag relatedness | 3.8 ms | 9462 | | a13 tag pairs | 45 ms | 336315 |
| q5 date window | 5.8 ms | 7898 | | a25 relatedness | 7.2 ms | 47499 |

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

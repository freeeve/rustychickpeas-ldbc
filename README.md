# rustychickpeas-ldbc

Loads **real** [LDBC](https://ldbcouncil.org/benchmarks/snb/) datasets into
[rustychickpeas](https://github.com/freeeve/rustychickpeas) — a CSR / RoaringBitmap
property-graph engine with **no query optimizer** — and times hand-coded queries against
them. The goal is a legitimate, reproducible, **laptop-scale** reading of graph
performance on real LDBC data.

It spans **five benchmark families** plus a Python-binding suite, each a thin
`src/bin/<family>.rs` over a shared loader/harness. This README is the **hub** —
per-family numbers, methodology, and reference-engine head-to-heads live in dedicated
**benchmark pages**:

| Family | What runs | Validation | Benchmark page |
|--------|-----------|------------|----------------|
| **BI** | 20 faithful analytical queries + 5 simplified patterns (SF1) | value-identical vs Kùzu on the cross-checkable subset | [docs/bench-bi.md](docs/bench-bi.md) |
| **Interactive (IC/IS)** | IC1–IC14 complex + IS1–IS7 short reads (SF1) | **20/20 value-identical vs Kùzu** | [docs/bench-interactive.md](docs/bench-interactive.md) |
| **Graphalytics** | BFS · PageRank · WCC · CDLP · LCC · SSSP | **PASS vs official reference outputs** | [docs/bench-graphalytics.md](docs/bench-graphalytics.md) |
| **SPB** | 30 SPARQL queries hand-translated to traversals (3.85 M triples) | **30/30 value-identical vs Oxigraph** | [docs/bench-spb.md](docs/bench-spb.md) |
| **FinBench** | 12 Transaction Complex Reads (SF10) | Rust-unit-tested; Kùzu value cross-check in progress | [docs/bench-finbench.md](docs/bench-finbench.md) |
| **Python suite** | all five families through the Python bindings | parity vs Rust refs | [python/README.md](python/README.md) |

The honesty caveat carries throughout: **correctness is cross-checked; magnitudes are
preliminary** — single-threaded, often on a loaded machine, vs multi-threaded reference
engines. Graphalytics and SPB are the strongest: they validate value-for-value against an
independent reference (LDBC reference outputs / Oxigraph SPARQL), not just result shape.
Each query is hand-coded — there is no query optimizer, so the timings reflect
rustychickpeas doing the actual scan + traversal + aggregation.

## Setup

```bash
./scripts/download_sf1.sh        # ~206 MB compressed, real LDBC SF1 (CSV)
cargo run --release --bin bi     # loads data/, runs the queries
```

Run any family with `cargo run --release --bin <bi|ic|graphalytics|spb_parity|finbench>`.
Path-depends on `../rustychickpeas/rustychickpeas-core` (sibling checkout); the committed
`Cargo.lock` is seeded from the core repo so the shared parquet/object_store dependency
tree resolves to versions known to build.

## Can we compare to other systems?

This is the honest state of play:

- **Audited public LDBC SNB BI results** are at **SF100–10000** on large/clustered
  hardware, measuring the full Q1–Q20 under an auditor. They aren't comparable to a
  laptop SF1 run.
- **Published single-node SF1 per-query numbers are scarce** — systems publish at SF100+
  because SF1 is the validation scale. So there's no clean public table to drop ours next
  to.
- **The realistic laptop-scale comparison is a local head-to-head**: run the *same*
  official queries on a reference engine (Kùzu is embeddable and ships the official LDBC
  implementation; Oxigraph for SPB's SPARQL) against the *same* data on the *same*
  machine. That sidesteps the published-number gap entirely. Those head-to-heads live on
  each family's benchmark page.

What's defensible to state: rustychickpeas ingests millions of nodes + rels from CSV in
seconds single-threaded, and runs the real queries (no optimizer) in single-digit-to-low-
hundreds of ms on SF1.

## Two benchmark surfaces drove core capabilities

Queries that hit a missing capability got it built upstream in `rustychickpeas-core` —
the recurring "a benchmark surfaces a gap we fix in core" story (details on each
benchmark page):

- **BI Q11 / Q19 / Q20** → per-rel property access during traversal
  (`relationships(node, dir, type)`), weighted shortest paths (`dijkstra`), and the
  `aggregate` kernel's `where_via` projected-property filter (Q12).
- **SPB full-text + geo** → an inverted index and a KD-tree, both returning `NodeSet` so
  they compose with label sets and traversal.

## Roadmap

All five families are implemented and validating, exposed to both the Rust binaries and
the Python bindings. Open threads:

1. **SPB editorial workload** — the insert/update/delete (write) queries; blocked on a
   core mutation/delta API (`GraphSnapshot` is currently immutable).
2. **SF10 across families** to line up with single-node academic numbers (already
   feasible for BI/IC; the data is present).
3. **Unloaded-machine timing pass** — every magnitude here is provisional until taken on a
   quiet box, with the single-threaded-vs-multi-threaded asymmetry settled.

## Layout

```
src/lib.rs              # shared library: re-exports loader/props/harness + bi
src/loader.rs           # CSV ingest (pipe-delimited gzip, per-type id maps,
                        #   message properties) -> GraphSnapshot + Stats
src/props.rs            # date arithmetic + typed property/graph accessors
src/harness.rs          # Result alias, JSON dump, median timing harness
src/{bi,interactive,graphalytics,spb,finbench}.rs   # the five families
src/bin/*.rs            # thin entry points -> <family>::run()
docs/bench-*.md         # per-family benchmark pages (numbers + methodology)
docs/families.md        # why these families, in what order (rationale/history)
python/                 # the Python-binding benchmark suite (python/README.md)
```

Query sources: official Cypher in
[`ldbc/ldbc_snb_bi/neo4j/queries`](https://github.com/ldbc/ldbc_snb_bi/tree/main/neo4j/queries);
SPARQL templates from the LDBC SPB; Graphalytics reference outputs from
[datasets.ldbcouncil.org](https://datasets.ldbcouncil.org/).

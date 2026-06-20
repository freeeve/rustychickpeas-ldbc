# rustychickpeas-ldbc

Loads **real** [LDBC](https://ldbcouncil.org/benchmarks/snb/) datasets into
[rustychickpeas](https://github.com/freeeve/rustychickpeas) — a CSR / RoaringBitmap
property-graph engine with **no query optimizer** — and times hand-coded queries
against them. The goal is a legitimate, reproducible, **laptop-scale** reading of
graph performance on real LDBC data.

It began with SNB BI and now spans **five benchmark families**, each a thin
`src/bin/<family>.rs` over a shared loader/harness:

| Family | What runs | Validation | Status |
|--------|-----------|------------|--------|
| **BI** | 20 faithful analytical queries + 5 simplified patterns (SF1) | value-identical vs Kùzu on the cross-checkable subset | ✅ |
| **Interactive (IC)** | IC1–IC14 complex + IS1–IS7 short reads (SF1) | **20/20 value-identical vs Kùzu** | ✅ |
| **Graphalytics** | BFS · PageRank · WCC · CDLP · LCC · SSSP | **PASS vs official reference outputs** (the one family with standardized validation) | ✅ |
| **SPB** | 30 SPARQL queries hand-translated to graph traversals (3.85 M triples) | **30/30 value-identical vs Oxigraph** | ✅ |
| **FinBench** | 12 Transaction Complex Reads (SF10) | vs Kùzu — see [`docs/finbench-results.md`](docs/finbench-results.md) | ✅ |

The same honesty caveat carries throughout: **correctness is cross-checked;
magnitudes are preliminary** (single-threaded, often on a loaded machine, vs
multi-threaded reference engines). Graphalytics and SPB are the strongest — they
validate value-for-value against an independent reference (LDBC reference outputs /
Oxigraph SPARQL), not just result shape.

The **BI** binary runs two query families:

- **Faithful BI queries** (`Q1`, `Q2`, …) — translations of the official
  [`ldbc/ldbc_snb_bi`](https://github.com/ldbc/ldbc_snb_bi) Cypher queries, with
  their real date parameters, filters and aggregations. These read stored node
  properties through the public graph API, so the timings reflect rustychickpeas
  doing the actual analytical work. There is **no query optimizer** — each query
  is hand-coded, single-threaded scan + traversal + aggregation.
- **Simplified patterns** (`BI1`–`BI6`) — the lighter namesakes the core repo's
  synthetic `ldbc_snb_bi` benchmark uses, kept for continuity with the
  synthetic-vs-real comparison.

## Setup

```bash
./scripts/download_sf1.sh        # ~206 MB compressed, real LDBC SF1 (CSV)
cargo run --release --bin bi     # loads data/, runs the queries
```

Path-depends on `../rustychickpeas/rustychickpeas-core` (sibling checkout); the
committed `Cargo.lock` is seeded from the core repo so the shared
parquet/object_store dependency tree resolves to versions known to build.

## Faithful queries implemented so far

| Query | Official definition | Needs |
|-------|---------------------|-------|
| **Q1 — Posting summary** | messages before a date (with content), grouped by (year, type, length-category) | message `creationDate`, `length`, `content` |
| **Q2 — Tag evolution** | for tags of a given TagClass, message counts in two consecutive 100-day windows | TagClass hierarchy, `hasTag`, message `creationDate` |
| **Q5 — Active posters** | for a tag, score each creator of tagged messages by 1·messages + 2·replies + 10·likes-received | `hasTag`, `hasCreator`, `likes`, `replyOf` |
| **Q6 — Authoritative users** | for a tag, score creators by the likes received by everyone who liked their tagged messages | `hasTag`, `hasCreator`, `likes` |
| **Q7 — Related topics** | for a tag, count distinct comments (replying to messages with that tag) by the *other* tags they carry | `hasTag`, `replyOf` |
| **Q8 — Central person** | score persons by tag interest (×100) + tagged messages in a window, plus their friends' scores | `hasInterest`, `hasTag`, `hasCreator`, `knows` |
| **Q9 — Thread initiators** | per person, count posts in a window and the messages in those posts' reply trees | `hasCreator`, `replyOf` tree, message dates |
| **Q11 — Friend triangles** | count triangles in the `knows` graph among a country's persons, with every rel created in a date window | location hierarchy, `knows` + **rel `creationDate`** |
| **Q13 — Zombies** | low-activity persons in a country, scored by share of likes coming from other zombies | location, person `creationDate`, `hasCreator`, `likes` |
| **Q12 — Message counts** | per person, count messages (content, length<thr, after date) whose root post's language is in a set; histogram persons by that count | `replyOf*0..`, `hasCreator`, post `language` |
| **Q19 — Interaction path** | weighted shortest path between people in two cities; rel weight = 1/(reply interactions) | location, `knows`, derived interaction weights, **dijkstra** |
| **Q20 — Recruitment** | weighted shortest path from a company's employees to a target person; rel weight = university-cohort closeness | `workAt`, `studyAt` (`classYear`), `knows`, **dijkstra** |

The table lists the first 12; **all 20 BI queries now run** (Q3/Q4/Q10/Q14–Q18
landed after this table was written), plus the 5 simplified patterns. Q8, Q11, Q19,
Q20 are **rustychickpeas-only** in the head-to-head for now (Q8 uses Neo4j pattern
comprehensions; Q11/Q19/Q20 need more of the schema loaded on the Kùzu side).

**These queries drove two core features.** Q11 filters `knows` rels by their
`creationDate` — per-rel property access *during traversal*, which the neighbor
accessors couldn't do (they return node ids, not the CSR position
`rel_prop` needs). That gap was closed upstream by
`GraphSnapshot::relationships(node, direction, type) -> RelationshipRef { …, pos }`.
Q19/Q20 are weighted shortest paths, which drove
`GraphSnapshot::dijkstra(source, …, weight) -> ShortestPaths` — the weight closure
reads the derived/rel-property cost via `rel.pos`, so it composes directly with
the relationship accessor. Exactly the kind of missing capability this exercise
was meant to surface and fix.

These use the example parameters from the official Cypher. Q2 surfaces real
artists (Rick_Springfield, Enrique_Iglesias, Freddie_Mercury), Q7 surfaces
plausible co-occurring tags, and Q19 finds 6 city-to-city interaction paths —
good smoke tests that the joins and weights are correct.

The Forum-hierarchy queries (Q3, Q4) and the involved social queries (Q10, Q14–Q18)
that were once deferred are now implemented and running (see the per-query timings
under the IC head-to-head and `results/sf1-results.txt`).

## Results — real LDBC SF1

Apple M3 Max, rustc 1.96.0, median of 5 runs after warmup.

The loaded subgraph: **2,887,110 nodes** (10,295 persons · 1,121,226 posts ·
1,739,438 comments · 16,080 tags · 71 tagclasses) and **6,042,860 rels**
(`hasCreator`/`hasTag`/`hasInterest`/`hasType`), built from gzipped CSV with
message properties in **~3 s**.

**Faithful BI queries:**

| Query | Time | Result |
|-------|------|--------|
| Q1 posting summary | ~57 ms | 12 groups over 691,085 messages before 2011-12-01 |
| Q2 tag evolution | ~155 ms | 100 MusicalArtist tags, two-window counts |

**Simplified patterns** (for synthetic-vs-real continuity):

| Query | Synthetic scale-50 | Real SF1 |
|-------|--------------------|----------|
| BI1 tag co-evolution | 123 ms | ~730 ms |
| BI3 popular topics | 99 ms | ~217 ms |
| BI4 top commenters | 18 ms | ~19 ms |
| BI5 active users | 7.2 ms | ~13 ms |

## Can we compare to other systems?

This is the honest state of play:

- **Audited public LDBC SNB BI results** are at **SF100–10000** on large/clustered
  hardware, measuring the full Q1–Q20 under an auditor. They are not comparable
  to a laptop SF1 run.
- **Published single-node SF1 per-query numbers are scarce** — systems publish at
  SF100+ because SF1 is the validation scale. So there is no clean public table
  to drop our numbers next to.
- **The realistic laptop-scale comparison is a local head-to-head**: run the
  *same* official Cypher on a reference engine (Kùzu is embeddable and ships the
  official LDBC BI implementation; or Neo4j) against the *same* SF1 data on the
  *same* machine. That sidesteps the published-number gap entirely and is the
  next concrete step (see below).

What's already defensible to state: rustychickpeas ingests 2.9 M nodes + 6 M
rels from CSV in ~3 s single-threaded, and runs the real Q1/Q2 (no optimizer)
in tens-to-hundreds of ms on SF1.

## Local head-to-head (Kùzu) — `kuzu/run.py`

`kuzu/run.py` is the reference side: it loads the *same* SFn data into
[Kùzu](https://kuzudb.com/) (embeddable, vectorized, columnar, with a real query
optimizer) and runs the same faithful queries. Kùzu has no label hierarchy, so we
project Post+Comment into one `Message` table; only Kùzu's COPY load and query
execution are timed (preprocessing is not). Setup:

```bash
python3 -m venv .venv-kuzu && .venv-kuzu/bin/pip install kuzu pandas
.venv-kuzu/bin/python kuzu/run.py <initial_snapshot_dir> sf10
```

**Correctness validated; magnitudes preliminary.** All four queries produce
identical result shapes on both engines (Q1 → 12 groups, Q2 → 100 tags,
Q7 → 100 tags, Q12 → 86 buckets), confirming they compute the same thing.

The interesting finding — robust even though these runs were taken on a loaded
machine, because the gaps are far larger than load noise — is that **the two
engines win opposite query shapes** (SF1, indicative ms):

| Query | shape | rustychickpeas | Kùzu | winner |
|-------|-------|---------------:|-----:|--------|
| Q1 posting summary | columnar aggregation scan | 57 | 3 | Kùzu (~18×) |
| Q2 tag evolution | aggregation + windows | 211 | 36 | Kùzu (~6×) |
| Q5 active posters | tag-scoped traversal + counts | 0.8 | 12 | rustychickpeas (~15×) |
| Q6 authoritative users | tag-scoped 2-hop likes | 555 | 790 | rustychickpeas (~1.4×) |
| Q7 related topics | targeted graph traversal | 4 | 57 | rustychickpeas (~14×) |
| Q12 message counts | recursive reply-chains | 322 | 41,645 | rustychickpeas (~130×) |

Read qualitatively, not to the digit: Kùzu's vectorized columnar engine wins the
full-scan aggregations (Q1, Q2); rustychickpeas's CSR/RoaringBitmap adjacency
wins the targeted traversal (Q7, which starts from one tag) and crushes the
recursive reply-chain walk (Q12) — where Kùzu's naive `replyOf*0..` translation
is pathologically slow (a better Kùzu formulation likely exists). **Precise
magnitudes still await an unloaded machine** (runs here had load average in the
tens; Kùzu is also multi-threaded by default while our queries are
single-threaded — an asymmetry to settle before quoting numbers).

### Interactive (IC) — `kuzu/run_ic.py`

The seed-anchored Interactive workload runs over the *same* SF1 snapshot (no
extra download): `cargo run --release --bin ic` on our side, `kuzu/run_ic.py
sf1` against the existing `db-sf1-faithful`, both using the seeds `pick_seeds`
chose. **All 20 cross-checkable queries are value-identical** across engines
(`kuzu/compare.py` over ic1–ic14 + is1/2/3/5/6/7 → ALL PASS, incl. 848 friend
rows).

Timings are SF1 under load avg ~6.7 on a shared box — read them **relatively**.
The Kùzu Cypher was optimized first (tasks 055–064) so this is a fair fight; the
initial reference Cypher was naive (correlated subqueries, `IN`-list membership,
un-deduped `knows*1..2`), e.g. IC4 6347→151 ms, IC5 4455→434 ms, IC6 2300→223 ms.

| IC query | rustychickpeas (ms) | Kùzu (ms) | winner |
|----------|--------------------:|----------:|--------|
| IC4 new topics | 7.0 | 151 | rustychickpeas ~22× |
| IC5 new groups | 896 | 434 | **Kùzu ~2×** |
| IC6 tag co-occurrence | 35 | 223 | rustychickpeas ~6× |
| IC7 recent likers | 1.6 | 89 | rustychickpeas ~55× |
| IC8 recent replies | 0.2 | 13 | rustychickpeas ~67× |
| IC9 recent FoF messages | 211 | 431 | rustychickpeas ~2× |
| IC10 friend recommendation | 4.3 | 1576 | rustychickpeas ~370× |
| IC12 expert search | 47 | 342 | rustychickpeas ~7× |
| IC13 unweighted shortest path | 5.1 | 3.7 | **Kùzu ~1.4×** |
| IC14 weighted shortest path | 11 | 6.7 | **Kùzu ~1.6×** |
| IS2/IS3/IS6/IS7 short reads | <0.2 | 1.6–29 | rustychickpeas |

(IC1 ~ties; IC2/IC3/IC11/IS1 favour rustychickpeas; full table in
`results/ic-sf1.txt`.) With fair Cypher it's a real race: chickpeas dominates the
short reads and CSR-friendly traversals (IS*, IC7/IC8/IC10 — neighbour iteration
vs query-engine overhead), while Kùzu's vectorized engine takes the native
shortest paths (IC13/IC14) and the heavy multi-hop aggregation (IC5). IC10's gap
is genuinely inherent (the 2-hop foaf expansion), not bad Cypher. The loader-backed half (IC1/IC3/IC5/IC7/IC10/IC11/IC12, IS1, IC14) is
cross-checked against a faithful import extended with the matching
rels/properties — additive, so BI stays 20/20 identical on the rebuilt
`db-sf1-faithful`. Only IS4 (content text, kept out of the shared faithful import
to keep BI loads lean) is not cross-checked. Full numbers: `results/ic-sf1.txt`.

## Graphalytics — `cargo run --release --bin graphalytics [dir] [name]`

The one family with **standardized validation**: LDBC Graphalytics ships a
reference output file per dataset × algorithm, so every run is a hard PASS/FAIL,
not a shape check. All six algorithms validate green, and the runner reports
**deterministic allocation counts** — the reliable signal, since wall-clock is
noisy on a shared box.

Real-scale, **wiki-Talk** (2.39 M nodes, 5.02 M rels), Apple M3 Max:

| Algorithm | Time | Allocations | Validation |
|-----------|-----:|------------:|------------|
| BFS | 64 ms | 2 | PASS |
| PageRank | 54 ms | 513 | PASS |
| WCC | 133 ms | 18 | PASS |
| CDLP | 170 ms | 715 | PASS |
| LCC | 1062 ms | 268 | PASS |
| SSSP | 6 ms | 4 | PASS¹ |

¹ wiki-Talk is unweighted, so SSSP runs unit-weight with no reference there; it
validates PASS on the weighted `example-directed`/`example-undirected` sets. The
near-constant allocation counts (BFS 2, WCC 18, SSSP 4) reflect pre-sized working
buffers over the CSR — there is no per-node allocation churn. Magnitudes are still
single-threaded laptop numbers, but the PASS/FAIL column is real validation.

## SPB (Semantic Publishing) — `cargo run --release --bin spb_parity`

SPB is RDF/SPARQL natively; we parse the N-Triples serialization, map it to a
property graph (IRI object → rel, literal → property, `rdf:type` → label), and
hand-translate the SPARQL templates into Rust traversals — no triple store, no
reasoner. The full-text (`full_text_search`) and geo queries run off two core
indexes (an inverted index + a KD-tree) that this family drove into
rustychickpeas-core.

**30/30 value-identical vs Oxigraph.** `scripts/spb_parity.py` runs the original
SPARQL against a local [Oxigraph](https://github.com/oxigraph/oxigraph) store over
the *same* 3.85 M-triple extract and diffs row-for-row against our results — every
query (q1–q9, a1–a25) MATCHES. Indicative timings (single run, M3 Max):

| Query | Time | Rows | | Query | Time | Rows |
|-------|-----:|-----:|-|-------|-----:|-----:|
| q1 minute histogram | 1.0 ms | 9457 | | a5 about-entity | 21 ms | 108476 |
| q9 fulltext union | 4.5 ms | 9462 | | a13 tag pairs | 40 ms | 336315 |
| q5 date window | 5.9 ms | 7898 | | a25 relatedness | 8.6 ms | 47499 |

(full 30-query table: the parity-script output + `results/spb.parity.rust.json`.)
This is the strongest correctness signal in the suite — value-identity against an
independent SPARQL engine, not a shape check.

## FinBench

Transaction-network schema (Account / `transfer` / `withdraw` / loan) with the 12
Transaction Complex Reads (TCR1–TCR12) — fraud-tracing-shape temporal-path and
fund-cycle queries that lean on the rel-`creationDate`-during-traversal capability
Q11 drove. All 12 are implemented and benchmarked head-to-head against Kùzu on
SF10; numbers, validation, and methodology live in
[`docs/finbench-results.md`](docs/finbench-results.md).

## Roadmap

All five families are implemented and validating. Open threads:

1. **SPB editorial workload** — the insert/update/delete (write) queries; blocked on
   a core mutation/delta API (`GraphSnapshot` is currently immutable).
2. **Expose the read primitives to other surfaces** — Python bindings
   (`rustychickpeas-python`) and the wasm reader (`rustychickpeas-reader`).
3. **SF10 across families** to line up with single-node academic numbers (already
   feasible for BI/IC; the data is present).
4. **Unloaded-machine timing pass** — every magnitude here is provisional until taken
   on a quiet box, with the single-threaded-vs-multi-threaded asymmetry settled.

## Layout

```
src/lib.rs              # shared library: re-exports loader/props/harness + bi
src/loader.rs           # CSV ingest (pipe-delimited gzip, per-type id maps,
                        #   message properties) -> GraphSnapshot + Stats
src/props.rs            # date arithmetic + typed property/graph accessors
src/harness.rs          # Result alias, JSON dump, median timing harness
src/bi/                 # faithful BI Q1-Q20 + simplified BI1-6, and run()
src/bin/bi.rs           # thin entry point -> rustychickpeas_ldbc::bi::run()
scripts/download_sf1.sh # fetch + extract real SF1 from the current LDBC mirror
results/sf1-results.txt # captured run
```

The loader and helpers live in the library so the IC / Graphalytics / FinBench / SPB
families (`docs/families.md`) share them as thin `src/bin/*.rs` binaries — all five
are implemented and wired in (`src/{interactive,graphalytics,spb,finbench}.rs` +
`src/bin/{bi,ic,graphalytics,spb,spb_parity,finbench}.rs`). Run a family with
`cargo run --release --bin <bi|ic|graphalytics|spb_parity|finbench>`.

Query sources: official Cypher in
[`ldbc/ldbc_snb_bi/neo4j/queries`](https://github.com/ldbc/ldbc_snb_bi/tree/main/neo4j/queries).

# rustychickpeas-ldbc

Loads the **real** [LDBC SNB BI](https://ldbcouncil.org/benchmarks/snb/) SF1
dataset into [rustychickpeas](https://github.com/freeeve/rustychickpeas) and
times analytical queries against it. The goal is a legitimate, reproducible,
**laptop-scale** reading of graph-analytics performance on real LDBC data.

Two query families run:

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
cargo run --release              # loads data/, runs the queries
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
| **Q11 — Friend triangles** | count triangles in the `knows` graph among a country's persons, with every edge created in a date window | location hierarchy, `knows` + **edge `creationDate`** |
| **Q13 — Zombies** | low-activity persons in a country, scored by share of likes coming from other zombies | location, person `creationDate`, `hasCreator`, `likes` |
| **Q12 — Message counts** | per person, count messages (content, length<thr, after date) whose root post's language is in a set; histogram persons by that count | `replyOf*0..`, `hasCreator`, post `language` |

Q8 and Q11 are **rustychickpeas-only** in the head-to-head for now: Q8's Cypher
leans on Neo4j pattern comprehensions that don't port cleanly to Kùzu, and Q11's
Kùzu side still needs the location + dated-`knows` schema loaded.

**Q11 drove a core feature.** It filters `knows` edges by their `creationDate`,
which needs per-edge property access *during traversal* — something the core's
neighbor accessors couldn't do (they return node ids, not the CSR edge position
that `relationship_property` needs). That gap was closed upstream by adding
[`GraphSnapshot::out_edges`](https://github.com/freeeve/rustychickpeas) (returns
`OutEdge { neighbor, rel_type, pos }`); Q11 reads each knows edge's date through
`pos`. This is exactly the kind of missing capability this exercise was meant to
surface and fix.

These use the example parameters from the official Cypher. Q2 surfaces real
artists (Rick_Springfield, Enrique_Iglesias, Freddie_Mercury) and Q7 surfaces
plausible co-occurring tags — good smoke tests that the joins are correct.

**Still deferred:** queries needing the Forum hierarchy (Q3, Q4, Q9), and the
weighted-shortest-path queries (Q15, Q19, Q20) which are out of scope for a graph
without a path-finding engine.

## Results — real LDBC SF1

Apple M3 Max, rustc 1.96.0, median of 5 runs after warmup.

The loaded subgraph: **2,887,110 nodes** (10,295 persons · 1,121,226 posts ·
1,739,438 comments · 16,080 tags · 71 tagclasses) and **6,042,860 edges**
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
edges from CSV in ~3 s single-threaded, and runs the real Q1/Q2 (no optimizer)
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

## Roadmap

1. **More faithful queries.** Q1/Q2 done. Next, in rough order of loader cost:
   - Q3/Q9 (Forum hierarchy: `containerOf`, `hasMember`, `hasModerator`)
   - Q12 (reply chains: `replyOf` transitive, Post `language`)
   - Q11/Q13/Q19 (`knows` graph + `isLocatedIn`/`isPartOf` location hierarchy)
2. **Local head-to-head** vs Kùzu/Neo4j on the same SF1 — the real comparison.
3. **SF10** to line up with single-node academic numbers (still laptop-feasible).

## Layout

```
src/main.rs             # CSV loader (pipe-delimited gzip, per-type id maps,
                        #   message properties) + faithful Q1/Q2 + simplified BI1-6
scripts/download_sf1.sh # fetch + extract real SF1 from the current LDBC mirror
results/sf1-results.txt # captured run
```

Query sources: official Cypher in
[`ldbc/ldbc_snb_bi/neo4j/queries`](https://github.com/ldbc/ldbc_snb_bi/tree/main/neo4j/queries).

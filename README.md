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
| **Q1 — Posting summary** | messages before a date (with content), grouped by (year, type, length-category); counts, avg/sum length, share of total | message `creationDate`, `length`, `content` |
| **Q2 — Tag evolution** | for tags of a given TagClass, message counts in two consecutive 100-day windows; report the difference | TagClass hierarchy, `hasTag`, message `creationDate` |

Both use the example parameters from the official Cypher
(`Q1: 2011-12-01`; `Q2: 2012-06-01, MusicalArtist`). Q2 surfaces real artists —
Rick_Springfield, Enrique_Iglesias, Freddie_Mercury — with their window counts,
a good smoke test that the join + windowing is correct.

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

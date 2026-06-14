# rustychickpeas-ldbc

Loads the **real** [LDBC SNB BI](https://ldbcouncil.org/benchmarks/snb/) SF1
dataset into [rustychickpeas](https://github.com/freeeve/rustychickpeas) and
times the BI-style analytical queries that the core repo's `ldbc_snb_bi`
benchmark otherwise runs against a *synthetic* graph. It exists to answer one
question honestly: **how do those query timings look on real LDBC data instead
of a generated stand-in?**

## Read this before quoting any numbers

These six queries (`BI1`–`BI6`) are **not** the official LDBC SNB BI workload.
The audited benchmark is a suite of ~20 queries (Q1–Q20) with date-range
filters, country/forum joins, multi-hop traversals and shortest paths. Ours are
simplified analytical patterns that share themes (tag popularity, top creators,
tag co-occurrence) but not the exact semantics. So:

- ✅ These numbers are a fair reading of **our** queries on **real** LDBC SF1
  data, with its real skew, hubs and cardinalities.
- ❌ They are **not** comparable to published LDBC SNB BI results, which measure
  the official Q1–Q20.
- ⚠️ rustychickpeas has **no query optimizer** — these are hand-coded,
  single-threaded scans + hashmap aggregation. The timings reflect raw
  scan/traversal throughput, not a planned query engine.

For a genuine published comparison you would implement the real Q1–Q20 and run
at SF10/SF100 (see [Future work](#future-work)).

## Setup

```bash
./scripts/download_sf1.sh        # ~206 MB compressed, real LDBC SF1 (CSV)
cargo run --release              # loads data/, runs the six queries
```

The project has a **path dependency** on `../rustychickpeas/rustychickpeas-core`,
so it expects the main repo checked out as a sibling directory. The committed
`Cargo.lock` is seeded from the core repo so the shared (parquet/object_store)
dependency tree resolves to versions known to build.

## Results — real LDBC SF1

Apple M3 Max, rustc 1.96.0, median of 5 runs after one warmup.

The loaded subgraph (Person/Post/Comment/Tag + `hasCreator`/`hasTag`/`hasInterest`
— the labels and edges these queries traverse; properties are skipped):

**2,887,039 nodes** (10,295 persons · 1,121,226 posts · 1,739,438 comments ·
16,080 tags) and **6,026,780 edges**, loaded from gzipped CSV in **~3 s**
(≈ 0.9 M nodes/s + edges, single-threaded parse + build).

| Query | What it does | Time |
|-------|--------------|------|
| BI1 — Tag co-evolution | tag pairs co-occurring across all posts & comments | ~740 ms |
| BI2 — Tag person path | persons (first 100) linked via shared interests | ~12 ms |
| BI3 — Popular topics | most-referenced tags (full scan + top-10) | ~220 ms |
| BI4 — Top commenters | persons with most comments (in-neighbor scan) | ~24 ms |
| BI5 — Active users | persons with most posts | ~15 ms |
| BI6 — Tag co-occurrence | frequently co-occurring tag pairs (posts) | ~170 ms |

The scan-heavy queries (BI1/BI3/BI6) walk all ~2.86 M messages and aggregate
into a hashmap each run; BI1 alone produces 3.36 M distinct tag pairs.

## Results — synthetic stand-in (for reference)

The core repo's `ldbc_snb_bi` benchmark at `LDBC_SYNTH_SCALE=50` builds a
generated graph of 1,110,000 nodes / 4,500,000 edges. Same queries, same
machine:

| Query | Synthetic (1.1 M / 4.5 M) | Real SF1 (2.9 M / 6.0 M) |
|-------|---------------------------|--------------------------|
| BI1 | 123 ms | ~740 ms |
| BI2 | 2.0 ms | ~12 ms |
| BI3 | 99 ms | ~220 ms |
| BI4 | 18 ms | ~24 ms |
| BI5 | 7.2 ms | ~15 ms |
| BI6 | 31 ms | ~170 ms |

Real SF1 is ~2.6× the nodes and has ~2.9× the messages, so the full-scan
queries (BI1/BI3/BI6) scale up roughly with message count — and the real tag
distribution produces far more distinct pairs, which is most of BI1's extra
cost. This is the main reason a synthetic stand-in under-reports work: it lacks
the real dataset's tag-popularity skew.

## How this compares to public benchmarks

Short version: it doesn't, and it shouldn't be presented as if it does.

- **Audited LDBC SNB BI** results (e.g. TigerGraph's Full Disclosure Reports)
  are run at **SF100 / SF1000 / SF10000** (100 GB – 10 TB) on large or
  multi-machine hardware, measuring the official Q1–Q20 under an independent
  auditor. SF1 (1 GB) is the *validation* scale, below what's published.
- **Single-node academic** numbers (the in-depth LDBC benchmarking paper, or
  systems like Kùzu/Umbra) sometimes report SF1/SF10 — that's the tier where a
  fair single-machine comparison would sit, but still against Q1–Q20.
- The legitimately comparable metric here is **ingest**: 2.9 M nodes + 6 M edges
  from CSV in ~3 s single-threaded is a reasonable engine-primitive number. The
  query times are best read as an internal regression signal, not a ranking.

Sources:
[LDBC SNB BI (VLDB 2023)](https://www.vldb.org/pvldb/vol16/p877-szarnyas.pdf) ·
[In-depth benchmarking (arXiv 1907.07405)](https://arxiv.org/pdf/1907.07405) ·
[LDBC datasets](https://ldbcouncil.org/benchmarks/snb/datasets/)

## Future work

1. Implement the actual LDBC SNB BI Q1–Q20 against this loader (the real
   comparison; substantial — the queries take parameters and have reference
   outputs to validate against).
2. Load the full schema (Forum, knows, likes, workAt, …), not just the BI1–6
   subset.
3. Run at SF10 to line up with single-node academic numbers.
4. Move off the core path-dependency once `rustychickpeas-core` is published to
   crates.io.

## Layout

```
src/main.rs            # CSV loader (pipe-delimited gzip, per-type i64->u32 maps) + queries
scripts/download_sf1.sh # fetch + extract real SF1 from the current LDBC mirror
results/sf1-results.txt # captured run
```

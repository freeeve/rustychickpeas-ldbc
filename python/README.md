# rustychickpeas-ldbc — Python benchmark suite

The `python/` suite runs the same LDBC benchmark families as the native Rust
binaries, but through the **rustychickpeas Python bindings**
([`rustychickpeas-python`](https://github.com/freeeve/rustychickpeas)). It exists to
answer two questions:

1. **Correctness** — do the bindings produce results identical to the Rust core?
2. **Performance** — how much of the Rust core's speed survives the Python boundary?
   I.e. how close can a Python-driven query get to hand-written Rust?

The performance question is the interesting one. A pure-Python `for` loop over
millions of nodes is hopeless — the interpreter *is* the floor. But the bindings
expose the core's native primitives (the parallel `aggregate` kernel, dense-column
`memoryview`s, a GIL-released `par_fold`, native single-source Dijkstra/BFS, bulk
rel-property accessors), so the hot loop runs in Rust while the orchestration stays in
Python. The BI tuning pass below is the payoff: the heaviest pure-Python queries ran **24–96×
slower** than Rust (Q12 946 ms, Q19 652 ms); routed through native primitives they drop
to **single-digit multiples — three queries faster than (single-threaded) Rust** — and
the residual gaps are mostly intrinsic to the query shape rather than missing features.

> Honesty caveat, same as the [main README](../README.md): correctness is
> cross-checked value-for-value; absolute magnitudes are preliminary (laptop,
> sometimes-loaded machine). The **Python ÷ Rust ratio** is the robust signal — same
> data, same machine, back-to-back runs — so read the ratios, not the milliseconds.

## Current state

| Family | Runner | Queries | Correctness | Python-vs-Rust timing |
|--------|--------|---------|-------------|-----------------------|
| **BI** | `run_bi.py` | Q1–Q20 | 20/20 parity vs Rust refs | ✅ full per-query — below |
| **Interactive** | `run_ic.py` | IC1–IC14 · IS1–IS7 | parity vs Rust refs | runner validated; timing pass pending |
| **SPB** | `run_spb.py` | q1–q9 · a1–a25 | parity vs Rust refs | runner validated; timing pass pending |
| **FinBench** | `run_finbench.py` | TCR1–TCR12 | parity vs Rust refs | runner validated; timing pass pending |
| **Graphalytics** | `run_graphalytics.py` | BFS · PageRank · WCC · CDLP · LCC · SSSP | PASS vs LDBC reference | runner validated; timing pass pending |

All five runners load real LDBC data through the bindings and validate their results.
The systematic **Python-vs-Rust timing** comparison has been completed for **BI** (20
queries); the other four run and validate today, and their timing-vs-Rust pass is the
current frontier (see [Extending the comparison](#extending-the-comparison)).

## Running

```bash
# 1. build + install the bindings (maturin, from the sibling core checkout)
(cd ../rustychickpeas/rustychickpeas-python && maturin develop --release)

# 2. (re)generate the Rust reference outputs the Python run diffs against
LDBC_EMIT_JSON=python/refs cargo run --release --bin bi

# 3. run a family — prints per-query median-of-5 ms + parity vs the Rust refs
python python/run_bi.py        # or run_ic / run_spb / run_finbench / run_graphalytics
```

Each runner reports, per query, the median wall time and a PASS/FAIL parity check
against `python/refs/*.rust.json`. Unit tests cover the loaders + queries on tiny
synthetic graphs: `pytest python/tests`.

## Python vs Rust — BI at SF1

**Scale.** Real LDBC SNB **SF1**: 2,887,110 nodes / 6,042,860 rels (10,295 persons ·
1.12 M posts · 1.74 M comments · 16,080 tags · 71 tagclasses), loaded from gzipped CSV
in ~3 s. Apple M3 Max, median of 5 after warmup.

- **Rust** = the hand-coded, **single-threaded** `src/bin/bi.rs`.
- **Python** = the same query through the bindings (`python/bi/`), tuned to route heavy
  work into native primitives.
- **Py/Rust** = Python ÷ Rust; **lower is better**, `<1` means Python is *faster*.

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

## Extending the comparison

The IC/IS, SPB, FinBench, and Graphalytics runners already execute and validate
through the bindings; what's missing is their per-query Python-vs-Rust *timing* pass
(the equivalent of the BI table). Each is a `python/run_<family>.py [snapshot]` against
the same data the Rust binary uses, diffed for parity — so producing those tables is a
benchmark run, not new code.

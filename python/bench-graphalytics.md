# Graphalytics — Python vs Rust (wiki-Talk)

[← Python suite](README.md) · related: [Graphalytics Rust + LDBC reference](../docs/bench-graphalytics.md)

How close does the **Python binding** get to **hand-written Rust** on the six
pure-topology Graphalytics algorithms — **BFS, PageRank, WCC, CDLP, LCC, SSSP**? Both run
the same algorithm over the same real-scale **wiki-Talk** graph (2,394,385 nodes); the Rust
side is `src/bin/graphalytics`, the Python side is `python/graphalytics/` through the
bindings over the shared CSR.

**Validation: 5/5 PASS against the official LDBC reference outputs** (`python
python/run_graphalytics.py data/graphalytics wiki-Talk`). This is the family with
*standardized* validation — a per-dataset reference file, so each run is a hard PASS/FAIL,
not a shape check. SSSP has no wiki-Talk reference (the graph is unweighted), so it is not
run here — matching the Rust page.

> **Numbers** are a single timed run, Apple M3 Max, ~3–4 cores of background load — wall
> time is noisy on a shared box (the PASS/FAIL is the reliable signal). **Py/Rust** =
> Python ÷ Rust; lower is better. **Rust column: provisional (re-bench pending)** — the
> Rust Graphalytics suite is mid-refactor; these are the current numbers from
> [bench-graphalytics.md](../docs/bench-graphalytics.md), to be refreshed when that lands.

| Algorithm | Python | Rust *(prov.)* | Py/Rust | Validation | how |
|-----------|-------:|---------------:|--------:|------------|-----|
| BFS | 588.5 ms | 64 ms | 9.2× | PASS | level-synchronous CSR frontier expansion |
| PageRank | 126.9 ms | 54 ms | 2.4× | PASS | iterated rank push over CSR (fixed iteration count) |
| WCC | 374.4 ms | 133 ms | 2.8× | PASS | union-find / label propagation over rels |
| CDLP | 666.8 ms | 170 ms | 3.9× | PASS | community label propagation (deterministic tie-break) |
| LCC | 2252.7 ms | 1062 ms | 2.1× | PASS | per-node neighbour-pair triangle counting |
| SSSP | — | 6 ms | — | n/a here | unit/weighted Dijkstra (no wiki-Talk reference) |

**Reading the table.** No sub-3 ms rows here — every algorithm touches all 2.4 M nodes, so
the ratios are real work, not call overhead. The iterative whole-graph algorithms cluster
at a tight **2–4×** (PageRank, WCC, CDLP, LCC): each iteration's hot loop runs over the
native CSR with a memoryview, so Python's residual cost is the per-iteration orchestration,
not per-node work — the closest the Python suite gets to Rust on real work outside BI.
**BFS is the outlier at 9×** — the frontier bookkeeping (visited set, per-level queues)
lives in Python rather than a single native traversal, the same gap the IC short-path
queries would close with a native BFS primitive exposed to the binding. The headline: all
five validate **PASS** against the LDBC reference value-for-value, and the bulk-iteration
algorithms land within 2–4× of hand-coded Rust because the per-iteration kernel is native.

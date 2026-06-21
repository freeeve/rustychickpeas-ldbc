# Graphalytics — LDBC Graphalytics

[← benchmark hub](../README.md) · related: [Graphalytics Python vs Rust](../python/bench-graphalytics.md)

The one family with **standardized validation**: LDBC Graphalytics ships a reference
output file per dataset × algorithm, so every run is a hard **PASS/FAIL**, not a shape
check. Six pure-topology algorithms — **BFS, PageRank, WCC, CDLP, LCC, SSSP** — a clean
showcase of CSR adjacency (no properties to scan, no optimizer needed).

```bash
cargo run --release --bin graphalytics [dir] [name]
```

> **Re-bench pending.** The timings below are from the prior pass, and the Rust
> Graphalytics suite is currently mid-refactor (uncommitted) — they'll be refreshed once
> that lands. The **PASS/FAIL** column is real validation and is not in question; the
> runner also reports deterministic allocation counts, the reliable signal since
> wall-clock is noisy on a shared box.

## Real-scale — wiki-Talk (2.39 M nodes, 5.02 M rels), Apple M3 Max

| Algorithm | Time | Allocations | Validation |
|-----------|-----:|------------:|------------|
| BFS | 64 ms | 2 | PASS |
| PageRank | 54 ms | 513 | PASS |
| WCC | 133 ms | 18 | PASS |
| CDLP | 170 ms | 715 | PASS |
| LCC | 1062 ms | 268 | PASS |
| SSSP | 6 ms | 4 | PASS¹ |

¹ wiki-Talk is unweighted, so SSSP runs unit-weight with no reference there; it validates
PASS on the weighted `example-directed`/`example-undirected` sets. The near-constant
allocation counts (BFS 2, WCC 18, SSSP 4) reflect pre-sized working buffers over the CSR —
no per-node allocation churn.

## Validation

All six algorithms validate green against the official reference outputs (BFS · PageRank ·
WCC · CDLP · LCC · SSSP). This is the strongest *standardized* signal in the suite — a
per-dataset reference file, so the comparison is value-for-value, not a shape check.

# 006 — Graphalytics runner (6 algorithms + validation)

**Goal.** A `src/bin/graphalytics.rs` that loads a Graphalytics dataset, runs the
six algorithms, validates against reference outputs, and times each.

**Why.** Two of the six already exist (`g.dijkstra` = SSSP; the connected-
components pass behind Q3/Q4/Q12 = WCC). This is mostly BFS + PageRank + CDLP +
LCC plus the loader and validator — and it showcases CSR adjacency with no
optimizer and no property scans.

**Depends on.** 001, 005. Stubs in `src/graphalytics.rs`.

**Files.**
- `src/bin/graphalytics.rs` + the `graphalytics` module
- a `.v`/`.e` -> `GraphBuilder` loader and a `.properties` parser
- a reference-output validator (exact for BFS/WCC/CDLP after relabelling;
  epsilon for PR/LCC/SSSP)

**Steps.**
1. Loader: parse `.v` (one vertex id per line) and `.e` (`src dst [weight]`) into
   a `GraphSnapshot`; read run params (source vertex, damping, iteration counts)
   from `.properties`.
2. Reuse SSSP (`g.dijkstra`) and WCC; implement `bfs`, `pagerank`, `cdlp`, `lcc`.
3. Validator: diff each output against the reference file with the spec
   tolerance; report PASS/FAIL per (dataset, algorithm).
4. Time each algorithm (median-of-5) via `time_query`.
5. Optional: the `knows`-subgraph self-export from `tasks/005` for a laptop-scale
   timing-only run on real SNB topology.

**Acceptance.**
- All six algorithms PASS on the example datasets.
- Per-algorithm timings printed; any approximated/self-generated run is labelled
  as timing-only (no silent "validated" claim without a reference file).

# 121 — Graphalytics foundation (loader + validator + bin)

Foundation for the Graphalytics family (LDBC spec v1.0.x, section 2.3).

- **Loader** (`graphalytics::load`): parse `<name>.v` (one vertex id/line),
  `<name>.e` (`src dst [weight]`), `<name>.properties` (directed flag + params:
  bfs/sssp source, pr damping + max-iterations, cdlp max-iterations). Build a
  `GraphSnapshot`: each vertex -> node, each rel -> a `e`-typed relationship
  with a `weight` f64 property; keep a `node -> original vertex id` map for
  output. Undirected: store each rel once; algorithms use Direction::Both.
- **Validator** (`graphalytics::validate`): diff a `Vec` output against the
  `<name>-<ALGO>` reference file — exact (BFS/CDLP), equivalence/relabel (WCC),
  epsilon 1e-6 (PR/LCC/SSSP).
- **Bin** `src/bin/graphalytics.rs`: load a dataset, run the six algorithms,
  validate (when reference outputs are present) and time each.

Data: zero-download path first (a small self-built or knows-subgraph graph,
unit-test correctness); chase an official mirror for reference outputs later.

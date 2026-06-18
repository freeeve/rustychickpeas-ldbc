# 005 — Graphalytics datasets + reference outputs

**Goal.** Get Graphalytics-format graphs and their reference output files so we
can validate, not just time.

**Why.** Graphalytics' big win over our current BI table is *standardized
validation*: every dataset ships expected per-algorithm outputs (`-BFS`, `-PR`,
`-WCC`, `-CDLP`, `-LCC`, `-SSSP`) and a `.properties` file of run parameters.

**Depends on.** 001.

**Files.**
- `scripts/download_graphalytics.sh` (created; verify the mirror URL)
- `data/graphalytics/<name>/` holding `<name>.v`, `<name>.e`, `<name>.properties`,
  and reference outputs

**Steps.**
1. Start with the tiny canonical sets (`example-directed`, `example-undirected`)
   to wire the validator, then a small real one (e.g. `wiki-Talk` / `kgs`).
2. **Verify the current download mirror** — the Graphalytics data location has
   moved over time; `download_graphalytics.sh` takes `GRAPHALYTICS_BASE_URL` so
   the path is confirmable, not hard-coded blindly.
3. Alternative, zero-download path: export our `knows` subgraph (Person-knows-
   Person) to `.v`/`.e` and self-generate a graph for laptop runs (no reference
   output, timing-only) — covered as an optional step in `tasks/006`.

**Acceptance.**
- At least the two example datasets present with `.v`/`.e`/`.properties` and
  reference outputs, loadable by the runner in `tasks/006`.

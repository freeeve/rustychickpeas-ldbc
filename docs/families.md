# LDBC benchmark families — beyond BI

Why these families, in what order, and how they fit rustychickpeas — a CSR /
RoaringBitmap property-graph engine with **no query optimizer**, where every
query is a hand-coded scan + traversal + aggregation.

## Rationale (read from the SF1 head-to-head)

The BI results split cleanly by query shape:

- **Kùzu's vectorized columnar engine wins full-scan aggregations** — Q1 (~18×),
  Q2 (~6×). Our row-at-a-time property scans can't beat a columnar reader here.
- **Our adjacency index wins targeted traversal** — Q7 (~14×, starts from one
  tag) and the recursive reply-chain walk Q12 (~130×).

So the next families are chosen to (a) lean into the traversal strength instead
of fighting the scan weakness, and (b) add the *standardized validation* the BI
table currently lacks ("magnitudes preliminary").

## Families

### Tier 1 — SNB Interactive (IC) — same schema, reuses our data
Seed-anchored short reads: 1–3 hop `knows` neighbourhoods, recent-message
lookups, two shortest-path queries (IC13 unweighted, IC14 weighted). This is the
exact shape we already win, and it reuses the **same `initial_snapshot` we
already load** — no new download. Complements BI: BI is the analytical/scan side
we lose, IC is the transactional/traversal side we win. Several IC queries map
directly onto code we already have (IC13/IC14 ≈ our Q19/Q20 dijkstra; IC1/IC9 ≈
`knows`-BFS). See `tasks/002`–`004`.

### Tier 1 — Graphalytics — we've already built half of it
Six pure-topology algorithms: **BFS, PageRank (PR), WCC, CDLP, LCC, SSSP**. We
already have WCC and SSSP (`g.dijkstra`, `knows_reachability`). No properties to
scan, no optimizer needed — a clean showcase of CSR adjacency. Crucially it ships
**reference output files per dataset+algorithm**, so we finally get pass/fail
validation and a deep pool of published cross-system numbers. See `tasks/005`–`006`.

### Tier 2 — FinBench — breadth, new schema + generator
Transaction-network schema (Account / transfer / withdraw / loan) with
temporal-path and fund-cycle queries (fraud-tracing shape). Plays to traversal
strength and to the edge-`creationDate`-during-traversal capability Q11 drove.
Heaviest lift: new schema, new Spark-based generator. See `tasks/007`–`008`.
All 12 Transaction Complex Reads (TCR1–TCR12) are implemented and benchmarked
head-to-head against Kùzu on SF10 — see [`finbench-results.md`](finbench-results.md).

### Tier 3 — SPB (Semantic Publishing) — feasible *and* drives two core features
SPB is RDF/SPARQL natively, but we run it with the **same trick as BI**: parse the
RDF *serialization* (N-Triples is line-parseable), map RDF -> property graph (IRI
object -> edge, literal object -> property, `rdf:type` -> label), and
hand-translate the SPARQL templates into Rust traversals. No SPARQL engine, no
triple store, no reasoner.

Its aggregation subset is scan-heavy (the shape we *lose* to columnar) — coverage
breadth, not a head-to-head win. But its **full-text and geo queries drive two
genuinely new core capabilities** into rustychickpeas-core — an inverted index
and a geo-spatial index — both returning `NodeSet` so they compose with label
sets and traversal. That's the same "a benchmark surfaces a missing capability we
fix upstream" story as the relationship accessor and `dijkstra`. See `tasks/010`
(loader + aggregation), `tasks/011`–`013`, and `docs/core-features.md`.

## Architecture decision (prerequisite)

`src/main.rs` is **2548 lines** — over our `<1000` worst-case guideline — and is a
**binary**, so its `load_graph` + property helpers (`pi64`, `pstr`, `parse_ms`,
`tag_by_name`, the `time_query` harness) cannot be imported by a sibling family.

Before adding families, extract `src/lib.rs`:

- `lib.rs` exposes the loader, `Stats`, the property/date helpers, and
  `time_query`.
- BI stays a thin bin (`src/bin/bi.rs`) calling the lib.
- One bin per family: `src/bin/{ic,graphalytics,finbench}.rs`.

The stub modules `src/{interactive,graphalytics,finbench,spb}.rs` already sketch
the per-family work; they are **not yet declared** in any crate root (cargo
ignores undeclared module files, so the build stays green) and get wired in by
`tasks/001`.

## Sequencing

```
001 extract lib  ─┬─▶ 002 IC seeds ─▶ 003 IC queries ─▶ 004 IC Kùzu ref ──────────┐
                  ├─▶ 005 Graphalytics data ─▶ 006 Graphalytics runner ───────────┤
                  ├─▶ 007 FinBench datagen ─▶ 008 FinBench queries ────────────────┼─▶ 009 results + docs
                  └─▶ 010 SPB loader + agg queries ─┐                              │
   011 core FTS index ─┬─────────────────────────────┴─▶ 013 SPB FTS+geo queries ─┘
   012 core geo index ─┘
```

001 is the only hard prerequisite for the families. SPB's FTS/geo queries (013)
additionally need the two core-feature tasks (011 FTS, 012 geo), which land in
`rustychickpeas-core` and are independent of each other. IC / Graphalytics /
FinBench / SPB are otherwise parallel. See `docs/core-features.md` for the core
API design.

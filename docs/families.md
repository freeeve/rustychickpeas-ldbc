# LDBC benchmark families — beyond BI

Why these families, in what order, and how they fit rustychickpeas — a CSR /
RoaringBitmap property-graph engine with **no query optimizer**, where every
query is a hand-coded scan + traversal + aggregation.

> **Status.** This is the original planning + rationale doc — kept for the *why*, not
> the numbers. All of it has shipped: tasks **001–013 are implemented and validating**
> (IC 20/20 vs Kùzu, Graphalytics 6/6 PASS vs reference, SPB 30/30 vs Oxigraph, FinBench
> TCR1–12 vs Kùzu), and the read primitives are now exposed to the **Python** and
> **wasm** surfaces too. For current numbers see the per-family benchmark pages
> ([bench-bi](bench-bi.md) · [bench-interactive](bench-interactive.md) ·
> [bench-graphalytics](bench-graphalytics.md) · [bench-spb](bench-spb.md) ·
> [bench-finbench](bench-finbench.md)) — not the figures below, which predate the
> query-optimization pass. Remaining open work is the SPB **editorial writes** (049–051,
> needs a core mutation API).

## Rationale (the *early* SF1 head-to-head)

> These figures are from the **first** BI head-to-head, before the query-optimization
> pass — they motivated the family choices, they are not current results. The pass
> since closed and *reversed* the scan gap: the native `aggregate` kernel now wins Q1
> and Q2 too. See [bench-bi](bench-bi.md) for current numbers.

At the time, the BI results split by query shape:

- **Kùzu's vectorized columnar engine won the full-scan aggregations** — Q1, Q2 —
  where our row-at-a-time property scans couldn't beat a columnar reader.
- **Our adjacency index won the targeted traversals** — Q7 (starts from one tag) and
  the recursive reply-chain walk Q12.

So the families were chosen to (a) lean into the traversal strength instead of fighting
the (then) scan weakness, and (b) add the *standardized validation* the BI table lacked.

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
strength and to the rel-`creationDate`-during-traversal capability Q11 drove.
Heaviest lift: new schema, new Spark-based generator. See `tasks/007`–`008`.
All 12 Transaction Complex Reads (TCR1–TCR12) are implemented and benchmarked
head-to-head against Kùzu on SF10 — see [`finbench-results.md`](finbench-results.md).

### Tier 3 — SPB (Semantic Publishing) — feasible *and* drives two core features
SPB is RDF/SPARQL natively, but we run it with the **same trick as BI**: parse the
RDF *serialization* (N-Triples is line-parseable), map RDF -> property graph (IRI
object -> rel, literal object -> property, `rdf:type` -> label), and
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

The original `src/main.rs` had grown past our `<1000`-line worst-case guideline and was
a **binary**, so its `load_graph` + property/date helpers and the `time_query` harness
couldn't be imported by a sibling family.

This was resolved by extracting `src/lib.rs` (`tasks/001`):

- `lib.rs` exposes the loader, `Stats`, the property/date helpers, and
  `time_query`.
- BI stays a thin bin (`src/bin/bi.rs`) calling the lib.
- One bin per family: `src/bin/{ic,graphalytics,finbench}.rs`.

The per-family modules `src/{interactive,graphalytics,finbench,spb}.rs` are
implemented and declared in the crate root, each driven by a thin
`src/bin/<family>.rs`. (They began as undeclared stubs and were wired in by
`tasks/001`, which extracted `src/lib.rs` as described above.)

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

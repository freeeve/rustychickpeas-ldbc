# Core features driven by SPB (full-text + geo)

The README's framing: *"these queries drove two core features"* (the relationship
accessor and `dijkstra`). SPB drives the next two — a **full-text index** and a
**geo-spatial index** in `rustychickpeas-core`. Both are implemented faithfully
(no approximation, no `tantivy`/`rstar` dependency) and both return `NodeSet` so
they compose with `nodes_with_label` and traversal like everything else.

Implementation lands in `../rustychickpeas/rustychickpeas-core/src/`; the SPB
benchmark (`tasks/013`) is the driver that exercises them.

## Grounding in the existing API

- `NodeSet` (`src/bitmap.rs`) — RoaringBitmap- or bitvec-backed; the result type
  for `nodes_with_label`, `relationships_with_type`. Has `iter/len/contains/
  as_range/par_fold/insert/remove` **and set algebra** (`BitAnd`/`BitOr`/`Sub` on
  `&NodeSet`).
- `GraphBuilder::finalize(index_properties: Option<&[&str]>)` — the **existing**
  hook that builds equality property indexes by scanning the typed columns
  (`node_col_str` / `_i64` / `_f64` / `_bool`). FTS/geo extend this same scan.
- `StringInterner` (`src/interner.rs`, lasso) — `get_or_intern`/`get`/`resolve`;
  reused as the FTS term dictionary. `Atoms` is its finalized snapshot form
  (`resolve` / `get_id`) holding full property values.
- `GraphSnapshot::prop(node, key) -> Option<ValueId>`, `ValueId::to_f64()` —
  read lat/lon literals for the geo index.

## Composition primitive: `NodeSet` set algebra — already present

FTS ∩ geo ∩ label composition needs intersect/union/difference. `NodeSet`
**already implements** these as operators on `&NodeSet` (`src/bitmap.rs`):
`BitAnd` (`&`), `BitOr` (`|`), and `Sub` (`-`), with a ≤256-element Bitset
fast-path. So composition works today with no core change:

```rust
let result = &fts_hits & &geo_hits & label_set;   // all NodeSet, native ops
```

Named `intersect`/`union`/`difference` wrappers would be optional readability
sugar over these operators — not a prerequisite.

## Feature 1 — Full-text index (`tasks/011`, `src/fulltext.rs`)

**Model.** A boolean inverted index with `NodeSet` postings, optionally layered
with BM25. The postings list for a term *is* a `NodeSet`, so query evaluation is
the existing `&`/`|`/`-` algebra; the term dictionary is a second
`StringInterner` (lasso). This is maximal reuse — see the model discussion in the
git history / task 011.

**Build.** A `GraphBuilder::index_fulltext(&[(label, property)])` registration;
at `finalize`, extend the same column scan the equality index uses — walk
`node_col_str[key]`'s `(node, str_id)` pairs, resolve each string, tokenize
(lowercase, split on non-alphanumeric, optional stopword drop; stemming deferred),
and add the node to each token's postings (`term -> NodeSet`). Term ids come from
a `StringInterner`. Store per-(term, doc) frequency + doc length only when BM25
ranking is enabled (gated, so the boolean path stays lean).

**Query.**
```rust
// boolean membership — default AND across query terms
pub fn fts(&self, field: FtsField, query: &str) -> NodeSet;
// BM25-ranked, top-k (uses df = postings.len(), tf, doc length, N)
pub fn fts_ranked(&self, field: FtsField, query: &str, k: usize) -> Vec<(NodeId, f32)>;
```

Boolean `fts` is enough for SPB's "creative works matching keyword"; `fts_ranked`
covers the relevance-ordered variants. Postings as RoaringBitmap means AND is a
bitmap intersect — the same primitive as the `NodeSet` algebra above.

**Deps:** none new (hand-rolled tokenizer + existing RoaringBitmap). No `tantivy`.

## Feature 2 — Geo-spatial index (`tasks/012`, `src/geo.rs`)

**Build.** A `GraphBuilder::index_geo(label, lat_property, lon_property)`; at
`finalize`, collect `(NodeId, lat, lon)` for nodes having both literals and build
a 2-D **k-d tree** (array-backed, median split — no deps). A uniform geohash grid
(`cell -> RoaringBitmap`) is the simpler alternative if bbox-only is enough.

**Query.**
```rust
pub fn geo_within_radius(&self, idx: GeoIndex, lat: f64, lon: f64, km: f64) -> NodeSet;
pub fn geo_within_bbox(&self, idx: GeoIndex, min: (f64, f64), max: (f64, f64)) -> NodeSet;
pub fn geo_knn(&self, idx: GeoIndex, lat: f64, lon: f64, k: usize) -> Vec<(NodeId, f64)>;
```

Radius search prunes by bounding box in the k-d tree, then filters with exact
**Haversine** great-circle distance (hand-rolled). Returns `NodeSet` to compose.

**Deps:** none new. No `geo` / `rstar`.

## The payoff — a faithful SPB geo+fts query

```rust
let near    = g.geo_within_radius(places, lat, lon, 50.0);     // places within 50 km
let works   = works_about(&g, &near);                          // about^-1 -> CreativeWork NodeSet
let matched = g.fts(cwork_body, "election");                   // keyword match
let result  = works.intersect(&matched);                       // composed, no SPARQL engine
```

This is the same composition style as the BI traversals — and it's the capability
that lets us run SPB's full-text and geo queries *faithfully* instead of
approximating them. Exactly the kind of missing capability this exercise surfaces
and fixes upstream.

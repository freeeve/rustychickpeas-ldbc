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
  as_range/par_fold/insert/remove`.
- `GraphBuilder::finalize(index_properties: Option<&[&str]>)` — the **existing**
  hook that builds equality property indexes. FTS/geo extend this pattern.
- `GraphSnapshot::prop(node, key) -> Option<ValueId>`, `ValueId::to_f64()` —
  read lat/lon literals. `Atoms` interner (`resolve` / `get_id`) — tokenize
  interned strings without re-allocating.

## Prerequisite: `NodeSet` set algebra

FTS ∩ geo ∩ label composition needs intersect/union/difference, which `NodeSet`
lacks today. RoaringBitmap has `&`/`|`/`-` natively; bitvec has bitwise ops — so
this is a thin wrapper.

```rust
impl NodeSet {
    pub fn intersect(&self, other: &NodeSet) -> NodeSet;
    pub fn union(&self, other: &NodeSet) -> NodeSet;
    pub fn difference(&self, other: &NodeSet) -> NodeSet;
}
```

## Feature 1 — Full-text index (`tasks/011`, `src/fulltext.rs`)

**Build.** A `GraphBuilder::index_fulltext(&[(label, property)])` registration;
at `finalize`, tokenize each node's string property (lowercase, split on
non-alphanumeric, optional stopword drop; stemming deferred) into an inverted
index `term -> RoaringBitmap<NodeId>`. Term dictionary interned. Store per-(term,
doc) frequency + doc length only if ranking is enabled.

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

# 012 — Core feature: geo-spatial index (rustychickpeas-core)

**Goal.** Add a faithful geo-spatial index to `rustychickpeas-core` so SPB's
"near a location" queries run for real — no `geo`/`rstar`, no approximation.

**Why.** SPB has geo-spatial aggregation (content about entities near a place).
This is the second core capability SPB drives upstream. Returns `NodeSet`, so it
composes with the full-text index (`tasks/011`) and label sets.

**Depends on.** 001; shares `NodeSet` set algebra with 011. Implemented in the
sibling crate.

**Files (in `../rustychickpeas/rustychickpeas-core/`).**
- new `src/geo.rs` — 2-D k-d tree + Haversine
- `src/graph_builder.rs` — `index_geo(label, lat_property, lon_property)`; build
  at `finalize` from `(NodeId, lat, lon)` triples
- `src/graph_snapshot.rs` — `geo_within_radius` / `geo_within_bbox` / `geo_knn`

**Steps.**
1. At `finalize`, read lat/lon via `prop(node, key)` + `ValueId::to_f64()` for
   nodes that have both; skip nodes missing either.
2. Build an array-backed k-d tree (median split, no recursion-depth surprises at
   SF scale). A geohash grid (`cell -> RoaringBitmap`) is the fallback if only
   bbox is needed.
3. `geo_within_radius` — bbox prune in the tree, then exact Haversine filter →
   `NodeSet`. `geo_within_bbox` — range search → `NodeSet`. `geo_knn` — bounded
   nearest search → `Vec<(NodeId, f64)>` with distances.
4. Tests: Haversine against known city-pair distances (tolerance), radius/bbox
   membership on a small fixture, k-NN ordering; fuzz coordinates incl. dateline
   and poles.

**Acceptance.**
- Radius/bbox/k-NN return correct results on a fixture; Haversine within
  tolerance of reference distances.
- No new third-party dependency added to core.
- `cargo test` green; coverage on the new module >80%.

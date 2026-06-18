# 013 — SPB full-text + geo queries (faithful, no approximation)

**Goal.** Implement the SPB query classes that need full-text and geo-spatial
search, using the new core indexes — the queries `tasks/010` deliberately left
out.

**Why.** With core FTS (`tasks/011`) and geo (`tasks/012`) in place, SPB's
full-text and geo queries become faithful translations, not "scan or drop." This
completes SPB coverage and demonstrates the two new core capabilities end-to-end.

**Depends on.** 010 (RDF loader + BGP/aggregation queries), 011 (FTS), 012 (geo).

**Files.**
- `src/bin/spb.rs` + the `spb` module (the `fts_*` / `geo_*` query fns)
- loader change: register `index_fulltext` on creative-work title/content and
  `index_geo` on place lat/lon at build time

**Steps.**
1. At load, call `index_fulltext` over creative-work text properties and
   `index_geo` over GeoNames place lat/lon.
2. Implement the full-text SPB queries via `g.fts(...)` (keyword search over
   creative works), composing with label/traversal `NodeSet`s.
3. Implement the geo SPB queries via `g.geo_within_radius(...)` /
   `geo_knn(...)`, then traverse `about^-1` to creative works.
4. Implement the combined geo+fts query (works near a place matching a keyword)
   as `geo_set` → traverse → `intersect(fts_set)`.
5. Smoke-test result shapes; time with `time_query`. Validate counts against a
   reference SPARQL run on a triple store (e.g. a one-off check), since SPB ships
   no per-query expected outputs the way Graphalytics does.

**Acceptance.**
- The full-text, geo, and combined geo+fts SPB queries run on the loaded RDF with
  stable, non-empty results — all faithful, none dropped.
- Timings printed; correctness spot-checked against a reference SPARQL engine and
  that check documented (validated vs timing-only stated explicitly).

//! SCAFFOLD — LDBC Semantic Publishing Benchmark (SPB). Not yet wired in.
//!
//! Inert until `tasks/001` extracts `src/lib.rs` and `tasks/010` builds the RDF
//! loader + queries. SPB is RDF/SPARQL *natively*, but we do NOT need a SPARQL
//! engine or a triple store — same trick as BI, where we hand-translate the
//! official Cypher into Rust traversals. Here we:
//!
//!   1. Parse the RDF **serialization** (N-Triples is line-oriented: `<s> <p>
//!      <o> .`) — a parser, not a store.
//!   2. Map RDF -> property graph: an IRI subject/object becomes a node; a
//!      predicate whose object is an IRI becomes a typed edge; a predicate whose
//!      object is a literal becomes a node property; `rdf:type` becomes a label.
//!   3. Hand-translate the SPARQL *aggregation* query templates (basic graph
//!      patterns + GROUP BY/ORDER BY/LIMIT + ontology-hierarchy traversal) into
//!      scan+traverse+aggregate, exactly like the BI queries.
//!
//! Full-text and geo are NOT approximated: SPB drives two new core features into
//! rustychickpeas-core — an inverted full-text index (`tasks/011`) and a
//! geo-spatial index (`tasks/012`) — so the FTS/geo queries below run faithfully
//! via `g.fts(...)` / `g.geo_within_radius(...)`, both returning `NodeSet` that
//! composes with label sets and traversal. See `docs/core-features.md`. Only
//! named graphs / RDFS entailment are flattened or materialized.

use rustychickpeas_core::GraphSnapshot;

/// SPB aggregation Q-style — creative works about a given reference entity
/// (e.g. a GeoNames place or DBpedia thing), within a date range, ordered by
/// recency. A plain BGP join: entity -> `about`^-1 creativeWork, filter date.
pub fn works_about_entity(
    _g: &GraphSnapshot,
    _entity: u32,
    _start_ms: i64,
    _end_ms: i64,
) -> Vec<u32> {
    todo!("traverse cwork-about-entity edges in reverse, filter dateModified")
}

/// SPB aggregation Q-style — roll a creative work's primary entity up the
/// reference ontology hierarchy (`rdfs:subClassOf` / category broader-than) and
/// count works per ancestor category. Same shape as BI Q2's TagClass climb.
pub fn works_by_category_rollup(_g: &GraphSnapshot, _root_category: u32) -> Vec<(u32, u64)> {
    todo!("transitive broader/subClassOf walk from root, count works per node")
}

/// SPB editorial mix — apply an insert of a creative work and its `about` links.
/// Updates are how SPB stresses the store; modelled here as a builder delta.
pub fn editorial_insert(_g: &GraphSnapshot, _cwork_triples: &[(u32, &str, u32)]) {
    todo!("stage RDF triples for a new creative work into the graph delta")
}

// --- Full-text + geo (tasks/011-013) — faithful, via new core indexes ---

/// SPB full-text Q-style — creative works whose title/content matches a keyword
/// query. Uses the core inverted index (`tasks/011`); intersect with the
/// CreativeWork label set to restrict.
///   `g.fts(cwork_body, query).intersect(g.nodes_with_label("CreativeWork")?)`
pub fn fts_works(_g: &GraphSnapshot, _query: &str) -> Vec<u32> {
    todo!("g.fts over creative-work text -> NodeSet, intersect label set")
}

/// SPB geo Q-style — creative works about a reference entity (e.g. a GeoNames
/// place) within `km` of a point. Uses the core geo index (`tasks/012`), then
/// traverses `about`^-1 to the works.
pub fn geo_works_near(_g: &GraphSnapshot, _lat: f64, _lon: f64, _km: f64) -> Vec<u32> {
    todo!("g.geo_within_radius -> places NodeSet -> about^-1 -> works")
}

/// SPB combined Q-style — works near a point AND matching a keyword. The whole
/// point of giving both indexes a `NodeSet` return type: compose by intersect.
pub fn geo_fts_works(_g: &GraphSnapshot, _lat: f64, _lon: f64, _km: f64, _query: &str) -> Vec<u32> {
    todo!("geo_within_radius -> about^-1 works, intersect fts(query)")
}

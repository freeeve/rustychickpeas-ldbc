//! SPB advanced **q6** — the most popular about-entity *types* among creative
//! works with a given live-coverage flag and audience.
//!
//! Hand translation of `advanced/aggregation_standard/query6.txt`:
//! ```sparql
//! SELECT ?aboutType (COUNT(*) AS ?count) WHERE {
//!   ?creativeWork cwork:about ?about ; cwork:liveCoverage {{{cwLiveCoverage}}} ;
//!     cwork:audience {{{cwAudience}}} .
//!   ?about a ?aboutType .
//! } GROUP BY ?aboutType ORDER BY DESC(?count) LIMIT 1000
//! ```
//!
//! `?about a ?aboutType` enumerates *every* type the about-target carries. The
//! about-targets are dbpedia entities (`dbo:Company` / `dbo:Event`), and the
//! loader forward-chains `rdfs:subClassOf coreconcepts:Thing`, so each target is
//! labelled both with its leaf type and with `Thing`. A single `(work, about)`
//! pair therefore contributes one row to the leaf bucket *and* one to the `Thing`
//! bucket. We reproduce the `COUNT(*)` over solution rows by iterating the entity
//! types `{Company, Event, Thing}` and counting, per type, the `(work, about)`
//! pairs whose about-target carries it (after the liveCoverage / audience filter).

use std::collections::HashMap;

use rustychickpeas_core::{Direction, GraphSnapshot};

use crate::props::{top_k_by_key, PropExt};

/// The entity types an about-target can carry: the two leaf classes plus the
/// materialized `coreconcepts:Thing` super-class (`rdfs:subClassOf` closure).
const ENTITY_TYPES: [&str; 3] = ["Company", "Event", "Thing"];

/// About-entity types ranked by how many `(work, about)` pairs they cover, over
/// works whose `liveCoverage` equals `live_coverage` and that carry an `audience`
/// edge to `audience_uri`. Returned as `(type_local_name, count)` ordered by count
/// descending then name, truncated to `limit` (the template's `LIMIT 1000`).
pub fn run(
    g: &GraphSnapshot,
    live_coverage: bool,
    audience_uri: &str,
    limit: usize,
) -> Vec<(String, usize)> {
    let Some(works) = g.nodes_with_label("CreativeWork") else {
        return Vec::new();
    };
    // Resolve each entity type's node set ONCE; the inner loop is then a bitmap
    // membership test rather than a per-node label string lookup.
    let type_sets: Vec<(&str, _)> = ENTITY_TYPES
        .iter()
        .filter_map(|&ty| g.nodes_with_label(ty).map(|s| (ty, s)))
        .collect();
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for w in works.iter() {
        if g.prop(w, "liveCoverage").bool_or(false) != live_coverage {
            continue;
        }
        let in_audience = g
            .neighbors_by_type(w, Direction::Outgoing, "audience")
            .any(|a| g.prop(a, "uri").str() == Some(audience_uri));
        if !in_audience {
            continue;
        }
        for about in g.neighbors_by_type(w, Direction::Outgoing, "about") {
            // ?about a ?aboutType — leaf type plus the materialized Thing super-class.
            for (ty, set) in &type_sets {
                if set.contains(about) {
                    *counts.entry(*ty).or_default() += 1;
                }
            }
        }
    }
    top_k_by_key(counts.into_iter().map(|(t, n)| (t.to_string(), n)), limit)
}

#[cfg(test)]
mod tests {
    use super::super::loader::load_str;
    use super::*;

    // TBox (Company/Event subClassOf coreconcepts:Thing) + works carrying a
    // liveCoverage flag, an audience edge, and an `about` edge to a typed entity:
    // two about a Company, one about an Event, plus one excluded by liveCoverage
    // and one excluded by audience.
    const FIXTURE: &str = r#"
<http://dbo/Company> <http://www.w3.org/2000/01/rdf-schema#subClassOf> <http://cc/Thing> .
<http://dbo/Event> <http://www.w3.org/2000/01/rdf-schema#subClassOf> <http://cc/Thing> .

<http://ex/cw1> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/CreativeWork> .
<http://ex/cw1> <http://bbc/liveCoverage> "true"^^<http://www.w3.org/2001/XMLSchema#boolean> .
<http://ex/cw1> <http://bbc/audience> <http://ex/Intl> .
<http://ex/cw1> <http://bbc/about> <http://ex/Acme> .

<http://ex/cw2> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/CreativeWork> .
<http://ex/cw2> <http://bbc/liveCoverage> "true"^^<http://www.w3.org/2001/XMLSchema#boolean> .
<http://ex/cw2> <http://bbc/audience> <http://ex/Intl> .
<http://ex/cw2> <http://bbc/about> <http://ex/Acme> .

<http://ex/cw3> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/CreativeWork> .
<http://ex/cw3> <http://bbc/liveCoverage> "true"^^<http://www.w3.org/2001/XMLSchema#boolean> .
<http://ex/cw3> <http://bbc/audience> <http://ex/Intl> .
<http://ex/cw3> <http://bbc/about> <http://ex/WorldCup> .

<http://ex/cw4> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/CreativeWork> .
<http://ex/cw4> <http://bbc/liveCoverage> "false"^^<http://www.w3.org/2001/XMLSchema#boolean> .
<http://ex/cw4> <http://bbc/audience> <http://ex/Intl> .
<http://ex/cw4> <http://bbc/about> <http://ex/Globex> .

<http://ex/cw5> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/CreativeWork> .
<http://ex/cw5> <http://bbc/liveCoverage> "true"^^<http://www.w3.org/2001/XMLSchema#boolean> .
<http://ex/cw5> <http://bbc/audience> <http://ex/Other> .
<http://ex/cw5> <http://bbc/about> <http://ex/Globex> .

<http://ex/Acme> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://dbo/Company> .
<http://ex/Globex> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://dbo/Company> .
<http://ex/WorldCup> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://dbo/Event> .
"#;

    #[test]
    fn counts_about_types_including_materialized_thing() {
        let g = load_str(FIXTURE).0;
        // liveCoverage=true, audience=Intl admits cw1, cw2 (about Acme=Company) and
        // cw3 (about WorldCup=Event); cw4 is excluded by liveCoverage, cw5 by
        // audience. Each about-target is also a Thing, so: Thing=3, Company=2,
        // Event=1.
        let rows = run(&g, true, "http://ex/Intl", 1000);
        assert_eq!(
            rows,
            vec![
                ("Thing".to_string(), 3),
                ("Company".to_string(), 2),
                ("Event".to_string(), 1),
            ]
        );
    }
}

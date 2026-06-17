//! SPB advanced **q5** — the most popular topics that creative works from either
//! of two categories are `about`.
//!
//! Hand translation of `advanced/aggregation_standard/query5.txt`:
//! ```sparql
//! SELECT ?about (COUNT(*) AS ?count) WHERE {
//!   ?creativeWork cwork:about ?about ; cwork:category ?category .
//!   ?about a {{{cwAboutEntityType}}} .
//!   FILTER ((?category = {{{cwCategoryType}}}) || (?category = {{{cwCategoryType}}})) .
//! } GROUP BY ?about ORDER BY DESC(?count) LIMIT 1000
//! ```
//!
//! `?about a {{{cwAboutEntityType}}}` is an entity-type restriction. The driver
//! samples a type such as `coreconcepts:Thing`, which the about-targets
//! (`dbo:Company` / `dbo:Event`) carry only by `rdfs:subClassOf` — the loader
//! forward-chains it, so the entity is labelled `Thing` and we restrict with
//! `has_label`. Counts each about-target over works whose `category` edge points
//! to either pinned uri.

use std::collections::HashMap;

use rustychickpeas_core::{Direction, GraphSnapshot};

use crate::props::pstr;

/// About-targets of the given entity-type `label` ranked by how many works
/// `category`-linked to `cat1` or `cat2` are about them. Returned as
/// `(entity_uri, count)` ordered by count descending then uri, truncated to
/// `limit` (the template's `LIMIT 1000`).
pub fn run(g: &GraphSnapshot, entity_label: &str, cat1: &str, cat2: &str, limit: usize) -> Vec<(String, usize)> {
    // Resolve the entity-type node set ONCE (the `?about a {{{entityType}}}`
    // restriction is then a bitmap test, not a per-node label string lookup).
    let (Some(entities), Some(works)) =
        (g.nodes_with_label(entity_label), g.nodes_with_label("CreativeWork"))
    else {
        return Vec::new();
    };
    let mut counts: HashMap<u32, usize> = HashMap::new();
    for w in works.iter() {
        let in_category = g.neighbors_by_type(w, Direction::Outgoing, "category").any(|c| {
            let u = pstr(g, c, "uri");
            u == Some(cat1) || u == Some(cat2)
        });
        if !in_category {
            continue;
        }
        for about in g.neighbors_by_type(w, Direction::Outgoing, "about") {
            if entities.contains(about) {
                *counts.entry(about).or_default() += 1;
            }
        }
    }
    // Sort / truncate on node ids, then resolve uris only for the kept rows.
    let mut rows: Vec<(u32, usize)> = counts.into_iter().collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    rows.truncate(limit);
    rows.into_iter().map(|(a, n)| (pstr(g, a, "uri").unwrap_or("?").to_string(), n)).collect()
}

#[cfg(test)]
mod tests {
    use super::super::loader::load_str;
    use super::*;

    // TBox (Company/Event subClassOf coreconcepts:Thing) + works about entities,
    // across two categories plus one excluded category.
    const FIXTURE: &str = r#"
<http://dbo/Company> <http://www.w3.org/2000/01/rdf-schema#subClassOf> <http://cc/Thing> .
<http://dbo/Event> <http://www.w3.org/2000/01/rdf-schema#subClassOf> <http://cc/Thing> .

<http://ex/cw1> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/CreativeWork> .
<http://ex/cw1> <http://bbc/category> <http://cat/Company> .
<http://ex/cw1> <http://bbc/about> <http://ex/Acme> .

<http://ex/cw2> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/CreativeWork> .
<http://ex/cw2> <http://bbc/category> <http://cat/Event> .
<http://ex/cw2> <http://bbc/about> <http://ex/Acme> .

<http://ex/cw3> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/CreativeWork> .
<http://ex/cw3> <http://bbc/category> <http://cat/Persons> .
<http://ex/cw3> <http://bbc/about> <http://ex/Globex> .

<http://ex/Acme> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://dbo/Company> .
<http://ex/Globex> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://dbo/Company> .
"#;

    #[test]
    fn counts_about_things_in_either_category() {
        let g = load_str(FIXTURE).0;
        // Categories Company + Event: cw1, cw2 (both about Acme, a Thing) -> 2.
        // cw3 (Persons category) excluded, so Globex never counted.
        let rows = run(&g, "Thing", "http://cat/Company", "http://cat/Event", 1000);
        assert_eq!(rows, vec![("http://ex/Acme".to_string(), 2)]);
    }
}

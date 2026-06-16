//! SPB-style queries, hand-translated against the RDF-as-property-graph mapping.
//!
//! These are the analogues of SPB's SPARQL aggregation templates, written as
//! scan + traversal over the property graph — the same approach the BI queries
//! take with Cypher. The full-text and geo queries use the core `fts` and
//! `geo_within_radius` capabilities (tasks 011/012); everything returns plain
//! node ids that compose. Vocabulary is referred to by RDF local name
//! (`CreativeWork`, `about`, `content`, `Place`, `lat`/`long`, `Concept`,
//! `broader`, `label`).

use std::collections::HashSet;

use rustychickpeas_core::{Direction, GraphSnapshot};

use crate::props::pstr;

/// Whether `node` carries the given label.
fn has_label(g: &GraphSnapshot, node: u32, label: &str) -> bool {
    g.nodes_with_label(label).is_some_and(|ns| ns.contains(node))
}

/// First node of `label` whose `label` (rdfs:label) property equals `name`.
fn node_by_name(g: &GraphSnapshot, label: &str, name: &str) -> Option<u32> {
    g.nodes_with_property(label, "label", name)?.iter().next()
}

/// The `rdfs:label` of a node, for display.
pub fn name_of(g: &GraphSnapshot, node: u32) -> &str {
    pstr(g, node, "label").unwrap_or("?")
}

/// SPB "creative works about an entity": works linked by `about` to the place
/// named `place_name`.
pub fn works_about_entity(g: &GraphSnapshot, place_name: &str) -> Vec<u32> {
    let Some(place) = node_by_name(g, "Place", place_name) else {
        return Vec::new();
    };
    g.neighbors_by_type(place, Direction::Incoming, "about")
        .filter(|&w| has_label(g, w, "CreativeWork"))
        .collect()
}

/// SPB "roll creative works up the category hierarchy": for the root category
/// and every descendant (reverse `broader`), count the creative works `about`
/// it. Mirrors BI Q2's TagClass climb. Returns `(category_name, work_count)`.
pub fn works_by_category_rollup(g: &GraphSnapshot, root_name: &str) -> Vec<(String, usize)> {
    let Some(root) = node_by_name(g, "Concept", root_name) else {
        return Vec::new();
    };
    // Collect the root and all narrower concepts (children point up via broader).
    let mut cats = vec![root];
    let mut seen: HashSet<u32> = HashSet::from([root]);
    let mut i = 0;
    while i < cats.len() {
        let c = cats[i];
        i += 1;
        for child in g.neighbors_by_type(c, Direction::Incoming, "broader") {
            if seen.insert(child) {
                cats.push(child);
            }
        }
    }
    cats.into_iter()
        .map(|c| {
            let count = g
                .neighbors_by_type(c, Direction::Incoming, "about")
                .filter(|&w| has_label(g, w, "CreativeWork"))
                .count();
            (name_of(g, c).to_string(), count)
        })
        .collect()
}

/// SPB full-text query: creative works whose `content` matches `query`
/// (boolean AND). Uses the core inverted index.
pub fn fts_works(g: &GraphSnapshot, query: &str) -> Vec<u32> {
    g.fts("CreativeWork", "content", query).iter().collect()
}

/// SPB geo query: creative works `about` a place within `km` of `(lat, lon)`.
/// Uses the core geo index, then traverses `about` in reverse.
pub fn geo_works_near(g: &GraphSnapshot, lat: f64, lon: f64, km: f64) -> Vec<u32> {
    let mut works: HashSet<u32> = HashSet::new();
    for place in g.geo_within_radius("Place", "lat", "long", lat, lon, km).iter() {
        for w in g.neighbors_by_type(place, Direction::Incoming, "about") {
            if has_label(g, w, "CreativeWork") {
                works.insert(w);
            }
        }
    }
    let mut out: Vec<u32> = works.into_iter().collect();
    out.sort_unstable();
    out
}

/// SPB combined query: creative works near `(lat, lon)` **and** matching
/// `query` — geo ∩ full-text.
pub fn geo_fts_works(g: &GraphSnapshot, lat: f64, lon: f64, km: f64, query: &str) -> Vec<u32> {
    let near: HashSet<u32> = geo_works_near(g, lat, lon, km).into_iter().collect();
    let mut hits: Vec<u32> = fts_works(g, query)
        .into_iter()
        .filter(|w| near.contains(w))
        .collect();
    hits.sort_unstable();
    hits
}

#[cfg(test)]
mod tests {
    use super::super::loader::load_str;
    use super::*;

    const FIXTURE: &str = r#"
<http://ex/cw1> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/CreativeWork> .
<http://ex/cw1> <http://bbc/content> "football in london" .
<http://ex/cw1> <http://bbc/about> <http://ex/Football> .
<http://ex/cw1> <http://bbc/about> <http://ex/London> .
<http://ex/cw2> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/CreativeWork> .
<http://ex/cw2> <http://bbc/content> "tennis in london" .
<http://ex/cw2> <http://bbc/about> <http://ex/Tennis> .
<http://ex/cw2> <http://bbc/about> <http://ex/London> .
<http://ex/cw3> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/CreativeWork> .
<http://ex/cw3> <http://bbc/content> "football in paris" .
<http://ex/cw3> <http://bbc/about> <http://ex/Football> .
<http://ex/cw3> <http://bbc/about> <http://ex/Paris> .
<http://ex/London> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://dbo/Place> .
<http://ex/London> <http://www.w3.org/2000/01/rdf-schema#label> "London" .
<http://ex/London> <http://geo#lat> "51.5074"^^<http://www.w3.org/2001/XMLSchema#double> .
<http://ex/London> <http://geo#long> "-0.1278"^^<http://www.w3.org/2001/XMLSchema#double> .
<http://ex/Paris> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://dbo/Place> .
<http://ex/Paris> <http://www.w3.org/2000/01/rdf-schema#label> "Paris" .
<http://ex/Paris> <http://geo#lat> "48.8566"^^<http://www.w3.org/2001/XMLSchema#double> .
<http://ex/Paris> <http://geo#long> "2.3522"^^<http://www.w3.org/2001/XMLSchema#double> .
<http://ex/Football> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://skos/Concept> .
<http://ex/Football> <http://www.w3.org/2000/01/rdf-schema#label> "Football" .
<http://ex/Football> <http://skos#broader> <http://ex/Sport> .
<http://ex/Tennis> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://skos/Concept> .
<http://ex/Tennis> <http://www.w3.org/2000/01/rdf-schema#label> "Tennis" .
<http://ex/Tennis> <http://skos#broader> <http://ex/Sport> .
<http://ex/Sport> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://skos/Concept> .
<http://ex/Sport> <http://www.w3.org/2000/01/rdf-schema#label> "Sport" .
"#;

    fn contents(g: &GraphSnapshot, works: &[u32]) -> Vec<String> {
        let mut c: Vec<String> = works
            .iter()
            .map(|&w| pstr(g, w, "content").unwrap_or_default().to_string())
            .collect();
        c.sort();
        c
    }

    #[test]
    fn about_entity_finds_works_for_a_place() {
        let (g, _) = load_str(FIXTURE);
        let works = works_about_entity(&g, "London");
        assert_eq!(works.len(), 2);
        assert_eq!(
            contents(&g, &works),
            ["football in london", "tennis in london"]
        );
        assert!(works_about_entity(&g, "Nowhere").is_empty());
    }

    #[test]
    fn category_rollup_counts_subtree() {
        let (g, _) = load_str(FIXTURE);
        let rollup = works_by_category_rollup(&g, "Sport");
        let total: usize = rollup.iter().map(|(_, n)| n).sum();
        assert_eq!(total, 3); // cw1+cw3 (Football) + cw2 (Tennis)
        let football = rollup.iter().find(|(n, _)| n == "Football").unwrap().1;
        assert_eq!(football, 2);
    }

    #[test]
    fn fulltext_over_content() {
        let (g, _) = load_str(FIXTURE);
        assert_eq!(contents(&g, &fts_works(&g, "football")), ["football in london", "football in paris"]);
        assert_eq!(contents(&g, &fts_works(&g, "tennis london")), ["tennis in london"]);
    }

    #[test]
    fn geo_near_and_combined_with_fulltext() {
        let (g, _) = load_str(FIXTURE);
        // 50 km of London catches works about London only (Paris is ~340 km).
        assert_eq!(
            contents(&g, &geo_works_near(&g, 51.5074, -0.1278, 50.0)),
            ["football in london", "tennis in london"]
        );
        // Widen to cover Paris too.
        assert_eq!(geo_works_near(&g, 51.5074, -0.1278, 500.0).len(), 3);
        // geo ∩ fts: near London AND mentioning tennis -> cw2 only.
        assert_eq!(
            contents(&g, &geo_fts_works(&g, 51.5074, -0.1278, 50.0, "tennis")),
            ["tennis in london"]
        );
    }
}

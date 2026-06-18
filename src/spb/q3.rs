//! SPB basic **q3** — creative works tagged with (i.e. `about` / `mentions`) a
//! given topic, freshest first, capped at a limit.
//!
//! Hand translation of `aggregation/query3.txt` (no SPARQL engine). The official
//! query SELECTs DISTINCT `?creativeWork` that carry `cwork:dateCreated ?created`
//! and `cwork:tag {{{cwAboutUri}}}` (the topic), constrained by UNIONs over
//! `primaryFormat` (Textual / Interactive / PictureGallery) and type
//! (NewsItem / BlogPost) plus an OPTIONAL `cwork:audience` filter, then
//! `ORDER BY DESC(?created)` and `LIMIT {{{randomLimit}}}`; the surrounding
//! CONSTRUCT merely re-emits each chosen work's properties.
//!
//! Mapping to the SPB-10 extract:
//!   * `cwork:tag` is the abstract tagging predicate; the BBC data materializes
//!     the concrete tag links as `about` (entities) and `mentions` (geonames
//!     Features), which our loader keeps as edges — so we union INCOMING `about`
//!     and `mentions` into the topic, matching the SPARQL's topic constraint.
//!   * `dateCreated` is an xsd:dateTime string literal; ISO-8601 sorts
//!     lexicographically, so DESC(?created) is a reverse string compare.
//!   * the `primaryFormat`, NewsItem / BlogPost type, and `audience` triples are
//!     not present in the extract, so those UNION/OPTIONAL filters are dropped —
//!     keeping them would reject every row. Flagged as a fidelity gap.
//!
//! Limitation: `about` targets are bare dbpedia resources, unlabelled in the
//! extract, so `node_by_uri` resolves only Features (`mentions` targets). Use a
//! Feature topic URI here.

use std::collections::HashSet;

use rustychickpeas_core::{Direction, GraphSnapshot};

use crate::props::{top_k_by_key, PropExt};

use super::queries::node_by_uri;

/// All creative works that tag (`about` or `mentions`) the topic at `topic_uri`
/// and carry a `dateCreated`, ordered newest-first by `dateCreated` (string,
/// DESC) with ties broken by node id, truncated to `limit`. Returns an empty
/// vector when the topic uri is unknown / unlabelled.
pub fn run(g: &GraphSnapshot, topic_uri: &str, limit: usize) -> Vec<u32> {
    let Some(topic) = node_by_uri(g, topic_uri) else {
        return Vec::new();
    };

    // Union of the two concrete tag links pointing INTO the topic, carrying each
    // work's `dateCreated` so the sort compares the strings directly (no
    // per-comparison `pstr`); `seen` dedups a work reached via both links.
    let Some(cworks) = g.nodes_with_label("CreativeWork") else {
        return Vec::new();
    };
    let mut out: Vec<(u32, &str)> = Vec::new();
    let mut seen: HashSet<u32> = HashSet::new();
    for pred in ["about", "mentions"] {
        for w in g.neighbors_in_set(topic, Direction::Incoming, pred, cworks) {
            if let Some(d) = g.prop(w, "dateCreated").str() {
                if seen.insert(w) {
                    out.push((w, d));
                }
            }
        }
    }

    top_k_by_key(out, limit)
        .into_iter()
        .map(|(w, _)| w)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::loader::load_str;
    use super::super::queries::name_of;
    use super::*;

    const LONDON: &str = "http://sws.geonames.org/london";

    // Three works mentioning London (dated Mar / Feb / Jan) plus one mentioning
    // Paris (newest, but a different topic — must be excluded).
    const FIXTURE: &str = r#"
<http://sws.geonames.org/london> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.geonames.org/ontology#Feature> .
<http://sws.geonames.org/london> <http://www.geonames.org/ontology#name> "London" .
<http://sws.geonames.org/paris> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.geonames.org/ontology#Feature> .
<http://sws.geonames.org/paris> <http://www.geonames.org/ontology#name> "Paris" .

<http://ex/jan> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/jan> <http://www.bbc.co.uk/ontologies/creativework/title> "Jan" .
<http://ex/jan> <http://www.bbc.co.uk/ontologies/creativework/dateCreated> "2024-01-10T08:00:00.000Z"^^<http://www.w3.org/2001/XMLSchema#dateTime> .
<http://ex/jan> <http://www.bbc.co.uk/ontologies/creativework/mentions> <http://sws.geonames.org/london> .

<http://ex/mar> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/mar> <http://www.bbc.co.uk/ontologies/creativework/title> "Mar" .
<http://ex/mar> <http://www.bbc.co.uk/ontologies/creativework/dateCreated> "2024-03-05T09:30:00.000Z"^^<http://www.w3.org/2001/XMLSchema#dateTime> .
<http://ex/mar> <http://www.bbc.co.uk/ontologies/creativework/mentions> <http://sws.geonames.org/london> .

<http://ex/feb> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/feb> <http://www.bbc.co.uk/ontologies/creativework/title> "Feb" .
<http://ex/feb> <http://www.bbc.co.uk/ontologies/creativework/dateCreated> "2024-02-20T12:00:00.000Z"^^<http://www.w3.org/2001/XMLSchema#dateTime> .
<http://ex/feb> <http://www.bbc.co.uk/ontologies/creativework/mentions> <http://sws.geonames.org/london> .

<http://ex/paris> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/paris> <http://www.bbc.co.uk/ontologies/creativework/title> "Paris" .
<http://ex/paris> <http://www.bbc.co.uk/ontologies/creativework/dateCreated> "2024-12-31T23:59:00.000Z"^^<http://www.w3.org/2001/XMLSchema#dateTime> .
<http://ex/paris> <http://www.bbc.co.uk/ontologies/creativework/mentions> <http://sws.geonames.org/paris> .
"#;

    fn titles_in_order(g: &GraphSnapshot, works: &[u32]) -> Vec<String> {
        works.iter().map(|&w| name_of(g, w).to_string()).collect()
    }

    #[test]
    fn orders_by_date_desc_and_excludes_other_topics() {
        let g = load_str(FIXTURE).0;
        // Newest-first over London's works; the (newer) Paris work is excluded.
        assert_eq!(
            titles_in_order(&g, &run(&g, LONDON, 10)),
            ["Mar", "Feb", "Jan"]
        );
    }

    #[test]
    fn limit_truncates_to_the_freshest() {
        let g = load_str(FIXTURE).0;
        assert_eq!(titles_in_order(&g, &run(&g, LONDON, 2)), ["Mar", "Feb"]);
        assert_eq!(run(&g, LONDON, 0).len(), 0);
    }

    #[test]
    fn unknown_topic_is_empty() {
        let g = load_str(FIXTURE).0;
        assert!(run(&g, "http://sws.geonames.org/atlantis", 10).is_empty());
    }

    #[test]
    fn ties_break_by_node_id() {
        // Two London works sharing one dateCreated; lower node id wins. Node ids
        // are assigned in first-appearance order, so `tie-a` (declared first) <
        // `tie-b`.
        const TIES: &str = r#"
<http://sws.geonames.org/london> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.geonames.org/ontology#Feature> .
<http://ex/tie-a> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/tie-a> <http://www.bbc.co.uk/ontologies/creativework/title> "A" .
<http://ex/tie-a> <http://www.bbc.co.uk/ontologies/creativework/dateCreated> "2024-05-01T00:00:00.000Z"^^<http://www.w3.org/2001/XMLSchema#dateTime> .
<http://ex/tie-a> <http://www.bbc.co.uk/ontologies/creativework/mentions> <http://sws.geonames.org/london> .
<http://ex/tie-b> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/tie-b> <http://www.bbc.co.uk/ontologies/creativework/title> "B" .
<http://ex/tie-b> <http://www.bbc.co.uk/ontologies/creativework/dateCreated> "2024-05-01T00:00:00.000Z"^^<http://www.w3.org/2001/XMLSchema#dateTime> .
<http://ex/tie-b> <http://www.bbc.co.uk/ontologies/creativework/mentions> <http://sws.geonames.org/london> .
"#;
        let g = load_str(TIES).0;
        assert_eq!(titles_in_order(&g, &run(&g, LONDON, 10)), ["A", "B"]);
    }
}

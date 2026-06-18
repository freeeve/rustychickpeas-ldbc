//! SPB basic **q1** — "retrieve creative works *about* thing `t` (or that
//! *mention* `t`), most-recently-modified first".
//!
//! Hand translation of `aggregation/query1.txt` (no SPARQL engine). The template
//! parameterizes one predicate (`cwork:about` *or* `cwork:mentions`) and one topic
//! IRI; we union both predicates. The official query's heavy `CONSTRUCT` projection
//! (titles, prefLabels, thumbnails, web documents, …) only decorates the rows the
//! inner sub-`SELECT` chooses, so the result *identity / order* is fully decided by:
//!
//! ```sparql
//! SELECT DISTINCT ?creativeWork WHERE {
//!   ?creativeWork {{cwAboutOrMentions}} {{topicUri}} .
//!   ?creativeWork a cwork:CreativeWork ; cwork:dateModified ?modified .
//! } ORDER BY DESC(?modified) LIMIT 10
//! ```
//!
//! which is what `run` reproduces: topic node -> incoming `about`/`mentions`
//! creative works -> order by `dateModified` (ISO-8601, so lexicographic) DESC.
//! `cwork:dateModified` is a required (non-`OPTIONAL`) pattern, so a work lacking
//! it cannot bind `?modified` and is excluded. The `LIMIT 10` is *not* applied
//! here — `run` returns the full ranked list and leaves truncation to the caller.

use std::collections::HashSet;

use rustychickpeas_core::{Direction, GraphSnapshot};

use super::queries::node_by_uri;
use crate::props::PropExt;

/// Creative works `about` OR `mentions` the topic at `topic_uri`, ranked by
/// `dateModified` descending (tie-broken by node id ascending for a stable order).
///
/// Returns an empty vector when the topic IRI is not a labelled node we keep
/// (see the module limitation note on unlabelled dbpedia `about` targets).
pub fn run(g: &GraphSnapshot, topic_uri: &str) -> Vec<u32> {
    let Some(topic) = node_by_uri(g, topic_uri) else {
        return Vec::new();
    };

    let Some(cworks) = g.nodes_with_label("CreativeWork") else {
        return Vec::new();
    };
    let mut works: HashSet<u32> = HashSet::new();
    for pred in ["about", "mentions"] {
        works.extend(g.neighbors_in_set(topic, Direction::Incoming, pred, cworks));
    }

    // The sub-SELECT's `cwork:dateModified ?modified` is required, so works with no
    // `dateModified` never reach the ORDER BY; carry the value to sort without re-lookup.
    let mut rows: Vec<(u32, &str)> = works
        .into_iter()
        .filter_map(|w| g.prop(w, "dateModified").str().map(|d| (w, d)))
        .collect();
    rows.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(&b.0)));
    rows.into_iter().map(|(w, _)| w).collect()
}

#[cfg(test)]
mod tests {
    use super::super::loader::load_str;
    use super::*;

    // Five creative works around a shared `london` Feature: two reach it by
    // `mentions`, one by `about` (exercising the union), one mentions a *different*
    // topic, and one mentions `london` but carries no `dateModified`.
    const FIXTURE: &str = r#"
<http://sws.geonames.org/london> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.geonames.org/ontology#Feature> .
<http://sws.geonames.org/london> <http://www.geonames.org/ontology#name> "London" .
<http://sws.geonames.org/paris> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.geonames.org/ontology#Feature> .
<http://sws.geonames.org/paris> <http://www.geonames.org/ontology#name> "Paris" .

<http://ex/cw1> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw1> <http://www.bbc.co.uk/ontologies/creativework/title> "Mentions June" .
<http://ex/cw1> <http://www.bbc.co.uk/ontologies/creativework/mentions> <http://sws.geonames.org/london> .
<http://ex/cw1> <http://www.bbc.co.uk/ontologies/creativework/dateModified> "2024-06-01T12:00:00.000+00:00"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/cw2> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw2> <http://www.bbc.co.uk/ontologies/creativework/title> "About September" .
<http://ex/cw2> <http://www.bbc.co.uk/ontologies/creativework/about> <http://sws.geonames.org/london> .
<http://ex/cw2> <http://www.bbc.co.uk/ontologies/creativework/dateModified> "2024-09-01T12:00:00.000+00:00"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/cw3> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw3> <http://www.bbc.co.uk/ontologies/creativework/title> "Mentions March" .
<http://ex/cw3> <http://www.bbc.co.uk/ontologies/creativework/mentions> <http://sws.geonames.org/london> .
<http://ex/cw3> <http://www.bbc.co.uk/ontologies/creativework/dateModified> "2024-03-01T12:00:00.000+00:00"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/cw4> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw4> <http://www.bbc.co.uk/ontologies/creativework/title> "Other topic" .
<http://ex/cw4> <http://www.bbc.co.uk/ontologies/creativework/mentions> <http://sws.geonames.org/paris> .
<http://ex/cw4> <http://www.bbc.co.uk/ontologies/creativework/dateModified> "2025-01-01T12:00:00.000+00:00"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/cw5> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw5> <http://www.bbc.co.uk/ontologies/creativework/title> "No date" .
<http://ex/cw5> <http://www.bbc.co.uk/ontologies/creativework/mentions> <http://sws.geonames.org/london> .
"#;

    fn titles(g: &GraphSnapshot, works: &[u32]) -> Vec<String> {
        works
            .iter()
            .map(|&w| g.prop(w, "title").str().unwrap_or("?").to_string())
            .collect()
    }

    #[test]
    fn orders_about_and_mentions_by_date_modified_desc() {
        let g = load_str(FIXTURE).0;
        let works = run(&g, "http://sws.geonames.org/london");
        // about + mentions union, newest `dateModified` first; the paris-topic work
        // and the `dateModified`-less work are excluded.
        assert_eq!(
            titles(&g, &works),
            ["About September", "Mentions June", "Mentions March"]
        );
    }

    #[test]
    fn unknown_topic_yields_no_rows() {
        let g = load_str(FIXTURE).0;
        assert!(run(&g, "http://sws.geonames.org/nowhere").is_empty());
    }
}

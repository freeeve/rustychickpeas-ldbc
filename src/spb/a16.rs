//! SPB advanced **q16** — creative works (with their tags and categories) whose
//! `title` contains a term, ordered by tag.
//!
//! Hand translation of `advanced/aggregation_standard/query16.txt` (no SPARQL
//! engine):
//! ```sparql
//! SELECT DISTINCT ?thing ?tag ?category ?title WHERE {
//!   ?thing rdf:type cwork:CreativeWork .
//!   ?thing rdf:type ?class . ?class rdfs:subClassOf cwork:CreativeWork .
//!   ?thing cwork:tag ?tag .
//!   ?thing ?a ?o . ?a rdfs:subPropertyOf cwork:tag .
//!   ?thing cwork:category ?category .
//!   ?thing cwork:title ?title .
//!   FILTER (CONTAINS(?title, "policy") && (?tag = ?o)) .
//!   OPTIONAL { ?thing cwork:audience ?audience . FILTER (?audience = {{{cwAudienceType}}}) . }
//! } ORDER BY ?tag LIMIT {{{randomLimit}}}
//! ```
//!
//! The reasoning the template asks for is exactly what the loader forward-chains.
//! `?class rdfs:subClassOf cwork:CreativeWork` is satisfied by every work's
//! subtype label (BlogPost/NewsItem/Programme), which materializes the
//! `CreativeWork` label our `fts` indexes on; and `cwork:tag` is the
//! super-property of `about`/`mentions`. The
//! `?thing ?a ?o . ?a rdfs:subPropertyOf cwork:tag . FILTER(?tag = ?o)` clause,
//! with `tag` already materialized as about∪mentions, collapses to one row per
//! `tag` target. `CONTAINS(?title, word)` is served by the core inverted index
//! (`fts`, whole-word — same caveat as q20/q21), and the required
//! `cwork:category`/`cwork:title` patterns demand a `category` edge and a
//! non-empty `title`. Rows are `(work_uri, tag_uri)`, deduped (`SELECT DISTINCT`),
//! ordered by tag then work (the template's `ORDER BY ?tag`), truncated to
//! `limit`.

use std::collections::BTreeSet;

use rustychickpeas_core::{Direction, GraphSnapshot};

use crate::props::pstr;

/// q16: for each creative work whose `title` matches `word` (full-text) and that
/// carries a `category` edge and a non-empty `title`, one `(work_uri, tag_uri)`
/// row per distinct `tag` target (about∪mentions), ordered by tag then work and
/// truncated to `limit`.
pub fn run(g: &GraphSnapshot, word: &str, limit: usize) -> Vec<(String, String)> {
    // Key the set by (tag_uri, work_uri) so iteration yields the `ORDER BY ?tag`
    // (work tie-break) ordering and `SELECT DISTINCT` dedup for free.
    let mut rows: BTreeSet<(String, String)> = BTreeSet::new();
    for w in g.fts("CreativeWork", "title", word).iter() {
        // A dense string property missing on a node reads back as Some(""); the
        // required `cwork:title ?title` pattern excludes such works.
        if g.str_prop(w, "title").is_none() {
            continue;
        }
        if g.neighbors_by_type(w, Direction::Outgoing, "category").next().is_none() {
            continue;
        }
        let Some(work_uri) = pstr(g, w, "uri") else {
            continue;
        };
        for tag in g.neighbors_by_type(w, Direction::Outgoing, "tag") {
            if let Some(tag_uri) = pstr(g, tag, "uri") {
                rows.insert((tag_uri.to_string(), work_uri.to_string()));
            }
        }
    }
    rows.into_iter().take(limit).map(|(tag, work)| (work, tag)).collect()
}

#[cfg(test)]
mod tests {
    use super::super::loader::load_str;
    use super::*;

    // TBox (BlogPost subClassOf CreativeWork; about/mentions subPropertyOf tag)
    // plus three BlogPosts: a "policy"-titled work with two tags (about + mentions)
    // and a category -> two rows; a non-"policy" work -> never matched; a
    // "policy"-titled work lacking a category -> excluded by the required pattern.
    const FIXTURE: &str = r#"
<http://www.bbc.co.uk/ontologies/creativework/BlogPost> <http://www.w3.org/2000/01/rdf-schema#subClassOf> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://www.bbc.co.uk/ontologies/creativework/about> <http://www.w3.org/2000/01/rdf-schema#subPropertyOf> <http://www.bbc.co.uk/ontologies/creativework/tag> .
<http://www.bbc.co.uk/ontologies/creativework/mentions> <http://www.w3.org/2000/01/rdf-schema#subPropertyOf> <http://www.bbc.co.uk/ontologies/creativework/tag> .

<http://ex/cw-policy> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/BlogPost> .
<http://ex/cw-policy> <http://www.bbc.co.uk/ontologies/creativework/title> "New policy on data sharing" .
<http://ex/cw-policy> <http://www.bbc.co.uk/ontologies/creativework/category> <http://www.bbc.co.uk/category/Politics> .
<http://ex/cw-policy> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/Data_protection> .
<http://ex/cw-policy> <http://www.bbc.co.uk/ontologies/creativework/mentions> <http://sws.geonames.org/london> .

<http://ex/cw-sport> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/BlogPost> .
<http://ex/cw-sport> <http://www.bbc.co.uk/ontologies/creativework/title> "Football match report" .
<http://ex/cw-sport> <http://www.bbc.co.uk/ontologies/creativework/category> <http://www.bbc.co.uk/category/Sport> .
<http://ex/cw-sport> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/Football> .

<http://ex/cw-nocat> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/BlogPost> .
<http://ex/cw-nocat> <http://www.bbc.co.uk/ontologies/creativework/title> "Policy without a category" .
<http://ex/cw-nocat> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/Governance> .
"#;

    #[test]
    fn policy_titled_work_emits_a_row_per_tag() {
        let g = load_str(FIXTURE).0;
        // cw-policy is about Data_protection and mentions london -> two tag rows,
        // ordered by tag uri. cw-sport (no "policy") and cw-nocat (no category) are
        // excluded, so they never appear.
        let rows = run(&g, "policy", 100);
        assert_eq!(
            rows,
            vec![
                ("http://ex/cw-policy".to_string(), "http://dbpedia.org/resource/Data_protection".to_string()),
                ("http://ex/cw-policy".to_string(), "http://sws.geonames.org/london".to_string()),
            ]
        );
    }

    #[test]
    fn limit_truncates_after_tag_order() {
        let g = load_str(FIXTURE).0;
        // LIMIT 1 keeps the first row in tag order (Data_protection < geonames).
        assert_eq!(
            run(&g, "policy", 1),
            vec![("http://ex/cw-policy".to_string(), "http://dbpedia.org/resource/Data_protection".to_string())]
        );
    }
}

//! SPB basic **q4** — "describe all blog posts tagged with a topic, newest first,
//! limited to a random 5–20 rows".
//!
//! Hand translation of `aggregation/query4.txt` (no SPARQL engine). The official
//! inner sub-SELECT decides identity and order:
//!
//! ```sparql
//! SELECT DISTINCT ?creativeWork {
//!   ?creativeWork cwork:tag {{topicUri}} ;
//!     cwork:dateCreated ?created ;
//!     cwork:primaryFormat {{cwFormat}} ;
//!     a {{cwType}} .
//! } ORDER BY DESC(?created) LIMIT {{randomLimit}}
//! ```
//!
//! In SPB `cwork:about` and `cwork:mentions` are sub-properties of `cwork:tag`, so
//! "tagged with the topic" expands (RDFS) to reaching the topic by `about` OR
//! `mentions`; with no RDFS engine we union those two edges directly. `a {{cwType}}`
//! is the blog-post subclass — a work carries BOTH the `CreativeWork` label and its
//! subclass label, so we filter `has_label("BlogPost")`. `cwork:dateCreated` is a
//! required pattern, giving the `ORDER BY DESC(?created)` key; `LIMIT` is `limit`.
//! Not reproduced: the `cwork:primaryFormat` filter and the outer describe-style
//! CONSTRUCT projection (we return ranked work ids).

use std::collections::HashSet;

use rustychickpeas_core::{Direction, GraphSnapshot};

use crate::props::{pstr, top_k_by_key};
use super::queries::node_by_uri;

/// Blog posts (`BlogPost`-labelled creative works) `about` OR `mentions` the topic
/// at `topic_uri`, ranked by `dateCreated` descending (tie-broken by node id
/// ascending) and truncated to `limit` rows.
///
/// Returns an empty vector when the topic IRI is not a labelled node we keep
/// (CreativeWork / Feature / Company / Event) or when nothing matches.
pub fn run(g: &GraphSnapshot, topic_uri: &str, limit: usize) -> Vec<u32> {
    let Some(topic) = node_by_uri(g, topic_uri) else {
        return Vec::new();
    };

    let Some(blogposts) = g.nodes_with_label("BlogPost") else {
        return Vec::new();
    };
    let mut posts: HashSet<u32> = HashSet::new();
    for pred in ["about", "mentions"] {
        posts.extend(g.neighbors_in_set(topic, Direction::Incoming, pred, blogposts));
    }

    // `cwork:dateCreated ?created` is required, so a post with no `dateCreated`
    // never reaches the ORDER BY; carry the value so the rank sorts without re-lookup.
    let rows = posts.into_iter().filter_map(|w| pstr(g, w, "dateCreated").map(|d| (w, d)));
    top_k_by_key(rows, limit).into_iter().map(|(w, _)| w).collect()
}

#[cfg(test)]
mod tests {
    use super::super::loader::load_str;
    use super::*;

    // Blog posts and one news item about a dbpedia `Company` topic (`Acme`), plus a
    // blog post about a different company and one with no `dateCreated`. Only the
    // `Acme` blog posts with a `dateCreated` should rank.
    const FIXTURE: &str = r#"
<http://dbpedia.org/resource/Acme> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://dbpedia.org/ontology/Company> .
<http://dbpedia.org/resource/Acme> <http://www.ldbcouncil.org/spb#prefLabel> "Acme Corp" .
<http://dbpedia.org/resource/Globex> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://dbpedia.org/ontology/Company> .

<http://ex/bp1> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/bp1> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/BlogPost> .
<http://ex/bp1> <http://www.bbc.co.uk/ontologies/creativework/title> "BP June" .
<http://ex/bp1> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/Acme> .
<http://ex/bp1> <http://www.bbc.co.uk/ontologies/creativework/dateCreated> "2024-06-01T12:00:00.000+00:00"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/bp2> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/bp2> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/BlogPost> .
<http://ex/bp2> <http://www.bbc.co.uk/ontologies/creativework/title> "BP September" .
<http://ex/bp2> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/Acme> .
<http://ex/bp2> <http://www.bbc.co.uk/ontologies/creativework/dateCreated> "2024-09-01T12:00:00.000+00:00"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/bp3> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/bp3> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/BlogPost> .
<http://ex/bp3> <http://www.bbc.co.uk/ontologies/creativework/title> "BP March" .
<http://ex/bp3> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/Acme> .
<http://ex/bp3> <http://www.bbc.co.uk/ontologies/creativework/dateCreated> "2024-03-01T12:00:00.000+00:00"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/ni1> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/ni1> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/NewsItem> .
<http://ex/ni1> <http://www.bbc.co.uk/ontologies/creativework/title> "News item" .
<http://ex/ni1> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/Acme> .
<http://ex/ni1> <http://www.bbc.co.uk/ontologies/creativework/dateCreated> "2025-01-01T12:00:00.000+00:00"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/bp4> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/bp4> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/BlogPost> .
<http://ex/bp4> <http://www.bbc.co.uk/ontologies/creativework/title> "Other topic" .
<http://ex/bp4> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/Globex> .
<http://ex/bp4> <http://www.bbc.co.uk/ontologies/creativework/dateCreated> "2025-05-01T12:00:00.000+00:00"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/bp5> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/bp5> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/BlogPost> .
<http://ex/bp5> <http://www.bbc.co.uk/ontologies/creativework/title> "No date" .
<http://ex/bp5> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/Acme> .
"#;

    fn titles(g: &GraphSnapshot, works: &[u32]) -> Vec<String> {
        works.iter().map(|&w| pstr(g, w, "title").unwrap_or("?").to_string()).collect()
    }

    #[test]
    fn blog_posts_about_topic_newest_first() {
        let g = load_str(FIXTURE).0;
        let works = run(&g, "http://dbpedia.org/resource/Acme", 10);
        // BlogPost-only, newest `dateCreated` first; the NewsItem, the other-topic
        // post, and the `dateCreated`-less post are all excluded.
        assert_eq!(titles(&g, &works), ["BP September", "BP June", "BP March"]);
    }

    #[test]
    fn limit_truncates_after_ordering() {
        let g = load_str(FIXTURE).0;
        let works = run(&g, "http://dbpedia.org/resource/Acme", 2);
        assert_eq!(titles(&g, &works), ["BP September", "BP June"]);
    }

    #[test]
    fn unknown_topic_yields_no_rows() {
        let g = load_str(FIXTURE).0;
        assert!(run(&g, "http://dbpedia.org/resource/Nope", 10).is_empty());
    }
}

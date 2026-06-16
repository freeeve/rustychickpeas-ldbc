//! SPB advanced **q8** — the most popular topics tagged by creative works of a
//! given type and audience whose `dateModified` falls in a window.
//!
//! Hand translation of `advanced/aggregation_standard/query8.txt`:
//! ```sparql
//! SELECT ?topic (COUNT(*) AS ?count) WHERE {
//!   ?creativeWork a {{{cwType}}} ; cwork:tag ?topic ;
//!     cwork:dateModified ?dt ; cwork:audience {{{cwAudience}}} .
//!   FILTER (?dt > {{{cwStartDateTime}}} && ?dt < {{{cwEndDateTime}}}) .
//! } GROUP BY ?topic ORDER BY DESC(?count)
//! ```
//!
//! `cwork:tag` is the RDFS super-property of `cwork:about` / `cwork:mentions`
//! (`rdfs:subPropertyOf`); the loader forward-chains it, so each `about`/`mentions`
//! statement is also a `tag` edge and we read the work's `tag` neighbours
//! directly — no per-query about/mentions fold. Counts each topic over the type /
//! audience / `dateModified` (exclusive, lexicographic) restrictions.

use std::collections::HashMap;

use rustychickpeas_core::{Direction, GraphSnapshot};

use crate::props::pstr;

/// Topics (`tag` targets) ranked by how many works of `cw_type` with audience
/// `audience_uri` and `dateModified` strictly within `(after, before)` tag them.
/// Returned as `(topic_uri, count)` ordered by count descending, then uri.
pub fn run(g: &GraphSnapshot, cw_type: &str, audience_uri: &str, after: &str, before: &str) -> Vec<(String, usize)> {
    let Some(works) = g.nodes_with_label(cw_type) else {
        return Vec::new();
    };
    let mut counts: HashMap<u32, usize> = HashMap::new();
    for w in works.iter() {
        let Some(dt) = pstr(g, w, "dateModified").filter(|s| !s.is_empty()) else {
            continue;
        };
        if !(dt > after && dt < before) {
            continue;
        }
        let audience = g
            .neighbors_by_type(w, Direction::Outgoing, "audience")
            .any(|a| pstr(g, a, "uri") == Some(audience_uri));
        if !audience {
            continue;
        }
        // cwork:tag — the materialized super-property of about/mentions.
        for topic in g.neighbors_by_type(w, Direction::Outgoing, "tag") {
            *counts.entry(topic).or_default() += 1;
        }
    }
    let mut rows: Vec<(String, usize)> = counts
        .into_iter()
        .map(|(t, n)| (pstr(g, t, "uri").unwrap_or("?").to_string(), n))
        .collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    rows
}

#[cfg(test)]
mod tests {
    use super::super::loader::load_str;
    use super::*;

    // TBox (about/mentions subPropertyOf tag) + two BlogPosts tagging topics via
    // about/mentions, one NewsItem (wrong type), and one out-of-window BlogPost.
    const FIXTURE: &str = r#"
<http://bbc/about> <http://www.w3.org/2000/01/rdf-schema#subPropertyOf> <http://bbc/tag> .
<http://bbc/mentions> <http://www.w3.org/2000/01/rdf-schema#subPropertyOf> <http://bbc/tag> .

<http://ex/cw1> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/BlogPost> .
<http://ex/cw1> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/CreativeWork> .
<http://ex/cw1> <http://bbc/about> <http://ex/Acme> .
<http://ex/cw1> <http://bbc/mentions> <http://ex/London> .
<http://ex/cw1> <http://bbc/audience> <http://ex/Intl> .
<http://ex/cw1> <http://bbc/dateModified> "2011-04-01T00:00:00.000Z"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/cw2> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/BlogPost> .
<http://ex/cw2> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/CreativeWork> .
<http://ex/cw2> <http://bbc/about> <http://ex/Acme> .
<http://ex/cw2> <http://bbc/audience> <http://ex/Intl> .
<http://ex/cw2> <http://bbc/dateModified> "2011-04-15T00:00:00.000Z"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/cw3> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/NewsItem> .
<http://ex/cw3> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/CreativeWork> .
<http://ex/cw3> <http://bbc/about> <http://ex/Acme> .
<http://ex/cw3> <http://bbc/audience> <http://ex/Intl> .
<http://ex/cw3> <http://bbc/dateModified> "2011-04-10T00:00:00.000Z"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/cw4> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/BlogPost> .
<http://ex/cw4> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/CreativeWork> .
<http://ex/cw4> <http://bbc/about> <http://ex/Acme> .
<http://ex/cw4> <http://bbc/audience> <http://ex/Intl> .
<http://ex/cw4> <http://bbc/dateModified> "2011-09-01T00:00:00.000Z"^^<http://www.w3.org/2001/XMLSchema#dateTime> .
"#;

    #[test]
    fn counts_tag_topics_over_type_audience_window() {
        let g = load_str(FIXTURE).0;
        let rows = run(&g, "BlogPost", "http://ex/Intl", "2011-03-01", "2011-06-01");
        // cw1 (about Acme + mentions London) and cw2 (about Acme) are in-window
        // BlogPosts; cw3 is a NewsItem, cw4 is out of window. So Acme=2, London=1.
        assert_eq!(
            rows,
            vec![("http://ex/Acme".to_string(), 2), ("http://ex/London".to_string(), 1)]
        );
    }
}

//! SPB advanced **q1** — the creative works that are `about` (or `mentions`) a
//! given thing, most-recently-modified first.
//!
//! Hand translation of `advanced/aggregation_standard/query1.txt`:
//! ```sparql
//! SELECT ?creativeWork ?modified WHERE {
//!   ?creativeWork a cwork:CreativeWork ;
//!     cwork:{{{aboutOrMentions}}} {{{cwThingURI}}} ;
//!     cwork:dateModified ?modified .
//! } ORDER BY DESC(?modified)
//! ```
//!
//! The work type is pinned by the materialized `CreativeWork` super-class — the
//! data types works as `BlogPost`/`NewsItem`/… subclasses, which the loader
//! forward-chains (`rdfs:subClassOf`). The `pred` argument is the sampled
//! predicate (`about` or `mentions`); we resolve the pinned thing by uri and read
//! its INCOMING `pred` neighbours, keeping the creative works that carry a
//! `dateModified`, ordered by it descending.

use rustychickpeas_core::{Direction, GraphSnapshot};

use super::queries::node_by_uri;

/// Creative works with a `pred` edge ("about" or "mentions") to the node with uri
/// `thing_uri` and a non-empty `dateModified`, ordered by `dateModified`
/// descending then node id ascending. Empty when the thing is unknown.
pub fn run(g: &GraphSnapshot, pred: &str, thing_uri: &str) -> Vec<u32> {
    let (Some(thing), Some(cworks)) = (
        node_by_uri(g, thing_uri),
        g.nodes_with_label("CreativeWork"),
    ) else {
        return Vec::new();
    };
    let mut works: Vec<(String, u32)> = Vec::new();
    for w in g.neighbors_in_set(thing, Direction::Incoming, pred, cworks) {
        let Some(modified) = g.prop_str(w, "dateModified") else {
            continue;
        };
        works.push((modified.to_string(), w));
    }
    works.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    works.into_iter().map(|(_, w)| w).collect()
}

#[cfg(test)]
mod tests {
    use super::super::loader::load_str;
    use super::*;
    use crate::props::PropExt;

    // TBox (BlogPost/NewsItem subClassOf CreativeWork) + a Company thing with two
    // dated about-works, one undated about-work (excluded), and one mentions-work.
    const FIXTURE: &str = r#"
<http://bbc/BlogPost> <http://www.w3.org/2000/01/rdf-schema#subClassOf> <http://bbc/CreativeWork> .
<http://bbc/NewsItem> <http://www.w3.org/2000/01/rdf-schema#subClassOf> <http://bbc/CreativeWork> .

<http://ex/Acme> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://dbo/Company> .

<http://ex/cw1> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/BlogPost> .
<http://ex/cw1> <http://bbc/about> <http://ex/Acme> .
<http://ex/cw1> <http://bbc/title> "Recent about" .
<http://ex/cw1> <http://bbc/dateModified> "2011-05-10T00:00:00.000Z"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/cw2> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/NewsItem> .
<http://ex/cw2> <http://bbc/about> <http://ex/Acme> .
<http://ex/cw2> <http://bbc/title> "Undated about" .

<http://ex/cw3> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/BlogPost> .
<http://ex/cw3> <http://bbc/about> <http://ex/Acme> .
<http://ex/cw3> <http://bbc/title> "Old about" .
<http://ex/cw3> <http://bbc/dateModified> "2011-01-01T00:00:00.000Z"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/cw4> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/BlogPost> .
<http://ex/cw4> <http://bbc/mentions> <http://ex/Acme> .
<http://ex/cw4> <http://bbc/title> "Mentions" .
<http://ex/cw4> <http://bbc/dateModified> "2012-01-01T00:00:00.000Z"^^<http://www.w3.org/2001/XMLSchema#dateTime> .
"#;

    fn titles(g: &GraphSnapshot, works: &[u32]) -> Vec<String> {
        works
            .iter()
            .map(|&w| g.prop(w, "title").str().unwrap_or("?").to_string())
            .collect()
    }

    #[test]
    fn about_works_ordered_by_modified_desc_undated_excluded() {
        let g = load_str(FIXTURE).0;
        // cw1 (2011-05-10) and cw3 (2011-01-01) are dated about-works; cw2 lacks a
        // dateModified so it is dropped; cw4 is a mentions-work, not an about-work.
        assert_eq!(
            titles(&g, &run(&g, "about", "http://ex/Acme")),
            ["Recent about", "Old about"]
        );
    }

    #[test]
    fn mentions_predicate_selects_the_mentions_work() {
        let g = load_str(FIXTURE).0;
        assert_eq!(
            titles(&g, &run(&g, "mentions", "http://ex/Acme")),
            ["Mentions"]
        );
    }

    #[test]
    fn unknown_thing_is_empty() {
        let g = load_str(FIXTURE).0;
        assert!(run(&g, "about", "http://ex/Nope").is_empty());
    }
}

//! SPB basic **query7** — date-range retrieval of creative works, with facets.
//!
//! Hand-translated from the official template
//! (`data/spb/.../aggregation/query7.txt`), no SPARQL engine:
//!
//! ```sparql
//! SELECT ?creativeWork ?dateCreated ?title ?category ?liveCoverage ?audience {
//!   ?creativeWork a {{{cwType}}} .
//!   ?creativeWork cwork:dateCreated  ?dateCreated .
//!   ?creativeWork cwork:title        ?title .
//!   ?creativeWork cwork:category     ?category .
//!   ?creativeWork cwork:liveCoverage ?liveCoverage .
//!   ?creativeWork cwork:audience     ?audience .
//!   {{{cwFilter…Condition}}}   # FILTER(?dateCreated >= after && ?dateCreated <= before)
//! }
//! ```
//!
//! `{{{cwType}}}` is the creative-work class — an `rdf:type`, i.e. a node
//! **label** under our loader; the date placeholder is a `[after, before]` range
//! FILTER (the "Date range query" choke point). `dateCreated` is an ISO-8601
//! string literal, so a lexicographic compare orders it. The BGP only yields a
//! row when `title` / `category` / `liveCoverage` / `audience` are all bound, so
//! we require those to be present. `category` / `audience` are edges to IRI
//! nodes; we optionally pin each to a target `uri` (a facet), otherwise we only
//! require the edge to exist (the SPARQL's "bound" semantics). The template has
//! no ORDER BY, so the result order is unspecified — we sort by node id for a
//! deterministic return.

use rustychickpeas_core::{Direction, GraphSnapshot};

use crate::props::pstr;

/// Whether `node` has an outgoing `edge` satisfying the facet:
/// * `Some(uri)` — at least one edge target carries that `uri`;
/// * `None` — at least one such edge exists at all (the SPARQL "bound" check).
fn facet_matches(g: &GraphSnapshot, node: u32, edge: &str, want_uri: Option<&str>) -> bool {
    match want_uri {
        None => g.has_edge(node, Direction::Outgoing, edge),
        Some(uri) => g.has_neighbor_with_property(node, Direction::Outgoing, edge, "uri", uri),
    }
}

/// SPB basic **q7**: creative works labelled `cw_type` whose `dateCreated` lies
/// in `[after, before]` (lexicographic on the ISO-8601 string), that carry a
/// `title` and a `liveCoverage`, and whose `category` / `audience` edges are
/// bound — optionally pinned to `category_uri` / `audience_uri`. Returns the
/// matching work ids, sorted by id.
pub fn run(
    g: &GraphSnapshot,
    cw_type: &str,
    after: &str,
    before: &str,
    category_uri: Option<&str>,
    audience_uri: Option<&str>,
) -> Vec<u32> {
    let Some(works) = g.nodes_with_label(cw_type) else {
        return Vec::new();
    };
    let mut out: Vec<u32> = works
        .iter()
        .filter(|&w| {
            let Some(created) = pstr(g, w, "dateCreated") else {
                return false;
            };
            created >= after
                && created <= before
                && pstr(g, w, "title").is_some()
                && g.prop(w, "liveCoverage").is_some()
                && facet_matches(g, w, "category", category_uri)
                && facet_matches(g, w, "audience", audience_uri)
        })
        .collect();
    out.sort_unstable();
    out
}

#[cfg(test)]
mod tests {
    use super::super::loader::load_str;
    use super::*;

    // Four creative works carrying dateCreated / title / liveCoverage and
    // category + audience edges. cw4 deliberately lacks an `audience` edge, so the
    // BGP's "audience bound" requirement must exclude it. Categories/audiences are
    // IRI nodes; the loader gives each a `uri` property we facet on.
    const FIXTURE: &str = r#"
<http://ex/cw1> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw1> <http://www.bbc.co.uk/ontologies/creativework/title> "Winter derby" .
<http://ex/cw1> <http://www.bbc.co.uk/ontologies/creativework/description> "a football match" .
<http://ex/cw1> <http://www.bbc.co.uk/ontologies/creativework/dateCreated> "2012-01-10T09:00:00.000+00:00"^^<http://www.w3.org/2001/XMLSchema#dateTime> .
<http://ex/cw1> <http://www.bbc.co.uk/ontologies/creativework/liveCoverage> "true"^^<http://www.w3.org/2001/XMLSchema#boolean> .
<http://ex/cw1> <http://www.bbc.co.uk/ontologies/creativework/category> <http://www.bbc.co.uk/category/sport> .
<http://ex/cw1> <http://www.bbc.co.uk/ontologies/creativework/audience> <http://www.bbc.co.uk/audience/national> .

<http://ex/cw2> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw2> <http://www.bbc.co.uk/ontologies/creativework/title> "Spring debate" .
<http://ex/cw2> <http://www.bbc.co.uk/ontologies/creativework/dateCreated> "2012-06-20T14:30:00.000+00:00"^^<http://www.w3.org/2001/XMLSchema#dateTime> .
<http://ex/cw2> <http://www.bbc.co.uk/ontologies/creativework/liveCoverage> "false"^^<http://www.w3.org/2001/XMLSchema#boolean> .
<http://ex/cw2> <http://www.bbc.co.uk/ontologies/creativework/category> <http://www.bbc.co.uk/category/politics> .
<http://ex/cw2> <http://www.bbc.co.uk/ontologies/creativework/audience> <http://www.bbc.co.uk/audience/international> .

<http://ex/cw3> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw3> <http://www.bbc.co.uk/ontologies/creativework/title> "Spring final" .
<http://ex/cw3> <http://www.bbc.co.uk/ontologies/creativework/dateCreated> "2013-03-05T08:00:00.000+00:00"^^<http://www.w3.org/2001/XMLSchema#dateTime> .
<http://ex/cw3> <http://www.bbc.co.uk/ontologies/creativework/liveCoverage> "true"^^<http://www.w3.org/2001/XMLSchema#boolean> .
<http://ex/cw3> <http://www.bbc.co.uk/ontologies/creativework/category> <http://www.bbc.co.uk/category/sport> .
<http://ex/cw3> <http://www.bbc.co.uk/ontologies/creativework/audience> <http://www.bbc.co.uk/audience/national> .

<http://ex/cw4> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw4> <http://www.bbc.co.uk/ontologies/creativework/title> "Mid report" .
<http://ex/cw4> <http://www.bbc.co.uk/ontologies/creativework/dateCreated> "2012-04-01T12:00:00.000+00:00"^^<http://www.w3.org/2001/XMLSchema#dateTime> .
<http://ex/cw4> <http://www.bbc.co.uk/ontologies/creativework/liveCoverage> "false"^^<http://www.w3.org/2001/XMLSchema#boolean> .
<http://ex/cw4> <http://www.bbc.co.uk/ontologies/creativework/category> <http://www.bbc.co.uk/category/sport> .
"#;

    const SPORT: &str = "http://www.bbc.co.uk/category/sport";
    const NATIONAL: &str = "http://www.bbc.co.uk/audience/national";

    fn titles(g: &GraphSnapshot, works: &[u32]) -> Vec<String> {
        let mut t: Vec<String> = works.iter().map(|&w| pstr(g, w, "title").unwrap_or("?").to_string()).collect();
        t.sort();
        t
    }

    #[test]
    fn date_range_requires_full_bgp() {
        let g = load_str(FIXTURE).0;
        // 2012 window, no facets: cw1 + cw2 (cw3 is 2013; cw4 lacks an audience).
        let hits = run(&g, "CreativeWork", "2012-01-01", "2012-12-31", None, None);
        assert_eq!(titles(&g, &hits), ["Spring debate", "Winter derby"]);
    }

    #[test]
    fn category_facet() {
        let g = load_str(FIXTURE).0;
        // 2012 + category=sport: cw1 only (cw2 is politics; cw4 sport but no audience).
        let hits = run(&g, "CreativeWork", "2012-01-01", "2012-12-31", Some(SPORT), None);
        assert_eq!(titles(&g, &hits), ["Winter derby"]);
    }

    #[test]
    fn audience_facet_over_full_range() {
        let g = load_str(FIXTURE).0;
        // all years + audience=national: cw1 + cw3 (cw2 international; cw4 unbound).
        let hits = run(&g, "CreativeWork", "2010-01-01", "2014-12-31", None, Some(NATIONAL));
        assert_eq!(titles(&g, &hits), ["Spring final", "Winter derby"]);
    }

    #[test]
    fn narrow_range_picks_one_year() {
        let g = load_str(FIXTURE).0;
        let hits = run(&g, "CreativeWork", "2013-01-01", "2013-12-31", None, None);
        assert_eq!(titles(&g, &hits), ["Spring final"]);
    }

    #[test]
    fn unknown_type_is_empty() {
        let g = load_str(FIXTURE).0;
        assert!(run(&g, "NoSuchType", "2000-01-01", "2099-12-31", None, None).is_empty());
    }
}

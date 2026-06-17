//! SPB advanced **query22** — faceted full-text search over creative works.
//!
//! Hand-translated from the official template
//! (`data/spb/.../advanced/aggregation_standard/query22.txt`), no SPARQL engine:
//!
//! ```sparql
//! SELECT {{{projection}}} {
//!   ?creativework a cwork:CreativeWork .
//!   ?creativework cwork:title         ?title .
//!   ?creativework cwork:description   ?description .
//!   ?creativework cwork:category      ?category .
//!   ?creativework cwork:tag           ?tag .
//!   ?creativework cwork:audience      ?audience .
//!   ?creativework cwork:liveCoverage  ?liveCoverage .
//!   ?creativework cwork:primaryFormat ?primaryFormat .
//!   ?creativework cwork:dateCreated   ?dateCreated .
//!   BIND (day(?dateCreated) as ?day) . BIND (year(?dateCreated) as ?year) . BIND (month(?dateCreated) as ?month) .
//!   {{{filter1}}} {{{filter2}}} {{{filter3}}}
//! }
//! {{{groupBy}}} {{{orderBy}}}
//! LIMIT 500
//! ```
//!
//! A faceted drill-down: a full-text search on a `word` in the creative work's
//! `title`, progressively narrowed by facets. `{{{filter1..3}}}` are the
//! substitution facets the drill-down pins; per the query header and the q22
//! facet set those are `category` / `audience` / `tag` (out-edges, matched by the
//! target node's `uri`), a `dateCreated` range (ISO-8601 string, lexicographic
//! compare), and `liveCoverage` (a bool). The BGP also binds `description` and
//! `primaryFormat`, so we require those present too (a row only exists when the
//! whole pattern is bound).
//!
//! We intersect the `title` full-text hits (core inverted index) with the
//! facet-filtered works, then `LIMIT`. `{{{groupBy}}}` / `{{{orderBy}}}` are
//! aggregation substitution parameters (group/order by year-month, tag, or
//! primary format depending on the iteration); we don't model a specific
//! grouping — we return the matching works deterministically id-ordered and
//! truncated to `limit`.

use rustychickpeas_core::{Direction, GraphSnapshot};

use crate::props::{pbool, pstr};

/// Whether `node` has an outgoing `edge` satisfying the facet:
/// * `Some(uri)` — at least one edge target carries that `uri`;
/// * `None` — at least one such edge exists at all (the SPARQL "bound" check).
fn facet_edge(g: &GraphSnapshot, node: u32, edge: &str, want_uri: Option<&str>) -> bool {
    match want_uri {
        None => g.has_rel(node, Direction::Outgoing, edge),
        Some(uri) => g.has_neighbor_with_property(node, Direction::Outgoing, edge, "uri", uri),
    }
}

/// The official `cwork:tag ?tag` requirement, folded to the `about`/`mentions`
/// topic links the SPB generator emits (it produces no literal `cwork:tag`):
/// `Some(uri)` matches an about/mentions edge to that uri; `None` requires at
/// least one such edge to exist.
fn folded_tag(g: &GraphSnapshot, node: u32, want_uri: Option<&str>) -> bool {
    facet_edge(g, node, "about", want_uri) || facet_edge(g, node, "mentions", want_uri)
}

/// SPB advanced **q22**: creative works whose `title` matches the full-text
/// `word`, that satisfy the full BGP (a bound `description`, `category`, `tag`,
/// `audience`, `liveCoverage`, `primaryFormat`, `dateCreated`), and that pass the
/// optional facets: `category` / `audience` / `tag` pinned to a target `uri`, a
/// `dateCreated` range `[after, before]`, and a `liveCoverage` value. Returns the
/// matching work ids, id-ordered and truncated to `limit` (the template's
/// `LIMIT 500`).
#[allow(clippy::too_many_arguments)]
pub fn run(
    g: &GraphSnapshot,
    word: &str,
    category_uri: Option<&str>,
    audience_uri: Option<&str>,
    tag_uri: Option<&str>,
    after: Option<&str>,
    before: Option<&str>,
    live_coverage: Option<bool>,
    limit: usize,
) -> Vec<u32> {
    let hits = g.fts("CreativeWork", "title", word);
    let mut out: Vec<u32> = hits
        .iter()
        .filter(|&w| {
            let Some(created) = pstr(g, w, "dateCreated") else {
                return false;
            };
            // Full BGP bound + the pinned facets (a `None` facet only requires the
            // property/edge to exist, matching the SPARQL's bound semantics).
            pstr(g, w, "description").is_some()
                && g.prop(w, "liveCoverage").is_some()
                && facet_edge(g, w, "primaryFormat", None)
                && facet_edge(g, w, "category", category_uri)
                && facet_edge(g, w, "audience", audience_uri)
                && folded_tag(g, w, tag_uri)
                && after.is_none_or(|a| created >= a)
                && before.is_none_or(|b| created <= b)
                && live_coverage.is_none_or(|lc| pbool(g, w, "liveCoverage") == lc)
        })
        .collect();
    out.sort_unstable();
    out.truncate(limit);
    out
}

#[cfg(test)]
mod tests {
    use super::super::loader::load_str;
    use super::*;

    // Creative works whose titles all contain "football" except cw3 (tennis).
    // cw4 deliberately lacks a `primaryFormat` edge, so the BGP must exclude it.
    const FIXTURE: &str = r#"
<http://ex/cw1> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw1> <http://www.bbc.co.uk/ontologies/creativework/title> "London football derby" .
<http://ex/cw1> <http://www.bbc.co.uk/ontologies/creativework/description> "a match report" .
<http://ex/cw1> <http://www.bbc.co.uk/ontologies/creativework/category> <http://www.bbc.co.uk/category/sport> .
<http://ex/cw1> <http://www.bbc.co.uk/ontologies/creativework/about> <http://www.bbc.co.uk/things/teamX> .
<http://ex/cw1> <http://www.bbc.co.uk/ontologies/creativework/audience> <http://www.bbc.co.uk/audience/national> .
<http://ex/cw1> <http://www.bbc.co.uk/ontologies/creativework/liveCoverage> "true"^^<http://www.w3.org/2001/XMLSchema#boolean> .
<http://ex/cw1> <http://www.bbc.co.uk/ontologies/creativework/primaryFormat> <http://www.bbc.co.uk/format/textual> .
<http://ex/cw1> <http://www.bbc.co.uk/ontologies/creativework/dateCreated> "2012-03-10T09:00:00.000+00:00"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/cw2> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw2> <http://www.bbc.co.uk/ontologies/creativework/title> "Paris football club" .
<http://ex/cw2> <http://www.bbc.co.uk/ontologies/creativework/description> "a transfer story" .
<http://ex/cw2> <http://www.bbc.co.uk/ontologies/creativework/category> <http://www.bbc.co.uk/category/sport> .
<http://ex/cw2> <http://www.bbc.co.uk/ontologies/creativework/about> <http://www.bbc.co.uk/things/teamY> .
<http://ex/cw2> <http://www.bbc.co.uk/ontologies/creativework/audience> <http://www.bbc.co.uk/audience/international> .
<http://ex/cw2> <http://www.bbc.co.uk/ontologies/creativework/liveCoverage> "false"^^<http://www.w3.org/2001/XMLSchema#boolean> .
<http://ex/cw2> <http://www.bbc.co.uk/ontologies/creativework/primaryFormat> <http://www.bbc.co.uk/format/video> .
<http://ex/cw2> <http://www.bbc.co.uk/ontologies/creativework/dateCreated> "2013-07-20T14:30:00.000+00:00"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/cw3> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw3> <http://www.bbc.co.uk/ontologies/creativework/title> "Wimbledon tennis final" .
<http://ex/cw3> <http://www.bbc.co.uk/ontologies/creativework/description> "a championship preview" .
<http://ex/cw3> <http://www.bbc.co.uk/ontologies/creativework/category> <http://www.bbc.co.uk/category/sport> .
<http://ex/cw3> <http://www.bbc.co.uk/ontologies/creativework/about> <http://www.bbc.co.uk/things/teamX> .
<http://ex/cw3> <http://www.bbc.co.uk/ontologies/creativework/audience> <http://www.bbc.co.uk/audience/national> .
<http://ex/cw3> <http://www.bbc.co.uk/ontologies/creativework/liveCoverage> "true"^^<http://www.w3.org/2001/XMLSchema#boolean> .
<http://ex/cw3> <http://www.bbc.co.uk/ontologies/creativework/primaryFormat> <http://www.bbc.co.uk/format/textual> .
<http://ex/cw3> <http://www.bbc.co.uk/ontologies/creativework/dateCreated> "2012-06-15T08:00:00.000+00:00"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/cw4> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw4> <http://www.bbc.co.uk/ontologies/creativework/title> "London football news" .
<http://ex/cw4> <http://www.bbc.co.uk/ontologies/creativework/description> "a news brief" .
<http://ex/cw4> <http://www.bbc.co.uk/ontologies/creativework/category> <http://www.bbc.co.uk/category/sport> .
<http://ex/cw4> <http://www.bbc.co.uk/ontologies/creativework/about> <http://www.bbc.co.uk/things/teamX> .
<http://ex/cw4> <http://www.bbc.co.uk/ontologies/creativework/audience> <http://www.bbc.co.uk/audience/national> .
<http://ex/cw4> <http://www.bbc.co.uk/ontologies/creativework/liveCoverage> "false"^^<http://www.w3.org/2001/XMLSchema#boolean> .
<http://ex/cw4> <http://www.bbc.co.uk/ontologies/creativework/dateCreated> "2012-09-01T12:00:00.000+00:00"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/cw5> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw5> <http://www.bbc.co.uk/ontologies/creativework/title> "Berlin football match" .
<http://ex/cw5> <http://www.bbc.co.uk/ontologies/creativework/description> "a fixture note" .
<http://ex/cw5> <http://www.bbc.co.uk/ontologies/creativework/category> <http://www.bbc.co.uk/category/politics> .
<http://ex/cw5> <http://www.bbc.co.uk/ontologies/creativework/about> <http://www.bbc.co.uk/things/teamZ> .
<http://ex/cw5> <http://www.bbc.co.uk/ontologies/creativework/audience> <http://www.bbc.co.uk/audience/national> .
<http://ex/cw5> <http://www.bbc.co.uk/ontologies/creativework/liveCoverage> "false"^^<http://www.w3.org/2001/XMLSchema#boolean> .
<http://ex/cw5> <http://www.bbc.co.uk/ontologies/creativework/primaryFormat> <http://www.bbc.co.uk/format/textual> .
<http://ex/cw5> <http://www.bbc.co.uk/ontologies/creativework/dateCreated> "2011-01-01T07:00:00.000+00:00"^^<http://www.w3.org/2001/XMLSchema#dateTime> .
"#;

    const SPORT: &str = "http://www.bbc.co.uk/category/sport";
    const NATIONAL: &str = "http://www.bbc.co.uk/audience/national";
    const TEAM_X: &str = "http://www.bbc.co.uk/things/teamX";

    fn titles(g: &GraphSnapshot, works: &[u32]) -> Vec<String> {
        let mut t: Vec<String> = works.iter().map(|&w| pstr(g, w, "title").unwrap_or("?").to_string()).collect();
        t.sort();
        t
    }

    #[test]
    fn fts_only_excludes_incomplete_bgp() {
        let g = load_str(FIXTURE).0;
        // "football" hits cw1/cw2/cw4/cw5; cw4 lacks primaryFormat -> dropped.
        let hits = run(&g, "football", None, None, None, None, None, None, 500);
        assert_eq!(titles(&g, &hits), ["Berlin football match", "London football derby", "Paris football club"]);
    }

    #[test]
    fn category_facet() {
        let g = load_str(FIXTURE).0;
        // football + category=sport: cw5 is politics -> only cw1, cw2.
        let hits = run(&g, "football", Some(SPORT), None, None, None, None, None, 500);
        assert_eq!(titles(&g, &hits), ["London football derby", "Paris football club"]);
    }

    #[test]
    fn audience_and_tag_facets() {
        let g = load_str(FIXTURE).0;
        // football + audience=national: cw1, cw5 (cw2 is international).
        let aud = run(&g, "football", None, Some(NATIONAL), None, None, None, None, 500);
        assert_eq!(titles(&g, &aud), ["Berlin football match", "London football derby"]);
        // football + tag=teamX: cw1 only (cw2 teamY, cw5 teamZ; cw3 teamX but not football).
        let tag = run(&g, "football", None, None, Some(TEAM_X), None, None, None, 500);
        assert_eq!(titles(&g, &tag), ["London football derby"]);
    }

    #[test]
    fn date_range_and_live_coverage_facets() {
        let g = load_str(FIXTURE).0;
        // football + dateCreated in 2012: cw1 (cw2 2013, cw5 2011).
        let dated = run(&g, "football", None, None, None, Some("2012-01-01"), Some("2012-12-31"), None, 500);
        assert_eq!(titles(&g, &dated), ["London football derby"]);
        // football + liveCoverage=true: cw1 only (cw2, cw5 are false).
        let live = run(&g, "football", None, None, None, None, None, Some(true), 500);
        assert_eq!(titles(&g, &live), ["London football derby"]);
    }

    #[test]
    fn limit_truncates() {
        let g = load_str(FIXTURE).0;
        assert_eq!(run(&g, "football", None, None, None, None, None, None, 500).len(), 3);
        // LIMIT 1 over the id-ordered set keeps cw1 (interned first).
        let one = run(&g, "football", None, None, None, None, None, None, 1);
        assert_eq!(titles(&g, &one), ["London football derby"]);
    }

    #[test]
    fn miss_returns_empty() {
        let g = load_str(FIXTURE).0;
        assert!(run(&g, "cricket", None, None, None, None, None, None, 500).is_empty());
    }
}

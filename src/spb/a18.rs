//! SPB advanced **query18** — date-range drill-down over modified creative works.
//!
//! Hand-translated from the official template
//! (`data/spb/.../advanced/aggregation_standard/query18.txt`), no SPARQL engine:
//!
//! ```sparql
//! SELECT ?cwork ?dateModif ?title ?category ?liveCoverage ?audience {
//!   ?cwork a {{{cwType}}} .
//!   ?cwork cwork:dateModified ?dateModif .
//!   ?cwork cwork:title        ?title .
//!   ?cwork cwork:category     ?category .
//!   ?cwork cwork:liveCoverage ?liveCoverage .
//!   ?cwork cwork:audience     ?audience .
//!   {{{cwFilterDateModifiedCondition}}}   # FILTER(?dateModif >= after && ?dateModif <= before)
//! }
//! {{{orderBy}}}                            # ORDER BY DESC(?dateModif)
//! LIMIT {{{randomLimit}}}
//! ```
//!
//! A "drill-down": retrieve the N most-recently-modified creative works in a
//! `[after, before]` window, then narrow/widen the window from a pick.
//! `{{{cwType}}}` is the work class — an `rdf:type`, i.e. a node **label** here.
//! `dateModified` is an ISO-8601 string literal, so a lexicographic compare both
//! orders and range-filters it. The BGP only yields a row when `title` /
//! `category` / `liveCoverage` / `audience` are all bound, so we require those to
//! be present (`category` / `audience` are edges to IRI nodes — only their
//! existence is checked; q18 does not facet on them). `{{{orderBy}}}` /
//! `{{{randomLimit}}}` are substitution parameters: we apply ORDER BY
//! DESC(?dateModif) (newest first — the drill-down idiom) with a node-id
//! tie-break for determinism, then `LIMIT`.

use rustychickpeas_core::{Direction, GraphSnapshot};

use crate::props::pstr;

/// Whether `node` has at least one outgoing `edge` (the SPARQL "bound" check for
/// the `category` / `audience` edges, which q18 projects but does not filter).
fn has_edge(g: &GraphSnapshot, node: u32, edge: &str) -> bool {
    g.neighbors_by_type(node, Direction::Outgoing, edge).next().is_some()
}

/// SPB advanced **q18**: the `limit` most-recently-modified creative works
/// labelled `cw_type` whose `dateModified` lies in `[after, before]`
/// (lexicographic on the ISO-8601 string) and that carry a `title`,
/// `liveCoverage`, and `category` + `audience` edges. Returns the matching work
/// ids ordered by `dateModified` descending (node id breaking ties), truncated
/// to `limit`.
pub fn run(g: &GraphSnapshot, cw_type: &str, after: &str, before: &str, limit: usize) -> Vec<u32> {
    let Some(works) = g.nodes_with_label(cw_type) else {
        return Vec::new();
    };
    let mut rows: Vec<(u32, &str)> = works
        .iter()
        .filter_map(|w| {
            let modified = pstr(g, w, "dateModified")?;
            let keep = modified >= after
                && modified <= before
                && pstr(g, w, "title").is_some()
                && g.prop(w, "liveCoverage").is_some()
                && has_edge(g, w, "category")
                && has_edge(g, w, "audience");
            keep.then_some((w, modified))
        })
        .collect();
    // ORDER BY DESC(?dateModif), then node id ascending for a stable tie-break.
    rows.sort_unstable_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(&b.0)));
    rows.into_iter().take(limit).map(|(w, _)| w).collect()
}

#[cfg(test)]
mod tests {
    use super::super::loader::load_str;
    use super::*;

    // Five creative works with varying dateModified. cwD deliberately lacks an
    // `audience` edge, so the BGP must exclude it regardless of its date.
    const FIXTURE: &str = r#"
<http://ex/cwA> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cwA> <http://www.bbc.co.uk/ontologies/creativework/title> "Alpha update" .
<http://ex/cwA> <http://www.bbc.co.uk/ontologies/creativework/dateModified> "2012-02-01T09:00:00.000+00:00"^^<http://www.w3.org/2001/XMLSchema#dateTime> .
<http://ex/cwA> <http://www.bbc.co.uk/ontologies/creativework/liveCoverage> "true"^^<http://www.w3.org/2001/XMLSchema#boolean> .
<http://ex/cwA> <http://www.bbc.co.uk/ontologies/creativework/category> <http://www.bbc.co.uk/category/sport> .
<http://ex/cwA> <http://www.bbc.co.uk/ontologies/creativework/audience> <http://www.bbc.co.uk/audience/national> .

<http://ex/cwB> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cwB> <http://www.bbc.co.uk/ontologies/creativework/title> "Bravo update" .
<http://ex/cwB> <http://www.bbc.co.uk/ontologies/creativework/dateModified> "2012-08-15T14:30:00.000+00:00"^^<http://www.w3.org/2001/XMLSchema#dateTime> .
<http://ex/cwB> <http://www.bbc.co.uk/ontologies/creativework/liveCoverage> "false"^^<http://www.w3.org/2001/XMLSchema#boolean> .
<http://ex/cwB> <http://www.bbc.co.uk/ontologies/creativework/category> <http://www.bbc.co.uk/category/politics> .
<http://ex/cwB> <http://www.bbc.co.uk/ontologies/creativework/audience> <http://www.bbc.co.uk/audience/international> .

<http://ex/cwC> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cwC> <http://www.bbc.co.uk/ontologies/creativework/title> "Charlie update" .
<http://ex/cwC> <http://www.bbc.co.uk/ontologies/creativework/dateModified> "2013-01-01T08:00:00.000+00:00"^^<http://www.w3.org/2001/XMLSchema#dateTime> .
<http://ex/cwC> <http://www.bbc.co.uk/ontologies/creativework/liveCoverage> "true"^^<http://www.w3.org/2001/XMLSchema#boolean> .
<http://ex/cwC> <http://www.bbc.co.uk/ontologies/creativework/category> <http://www.bbc.co.uk/category/sport> .
<http://ex/cwC> <http://www.bbc.co.uk/ontologies/creativework/audience> <http://www.bbc.co.uk/audience/national> .

<http://ex/cwD> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cwD> <http://www.bbc.co.uk/ontologies/creativework/title> "Delta update" .
<http://ex/cwD> <http://www.bbc.co.uk/ontologies/creativework/dateModified> "2012-05-05T12:00:00.000+00:00"^^<http://www.w3.org/2001/XMLSchema#dateTime> .
<http://ex/cwD> <http://www.bbc.co.uk/ontologies/creativework/liveCoverage> "false"^^<http://www.w3.org/2001/XMLSchema#boolean> .
<http://ex/cwD> <http://www.bbc.co.uk/ontologies/creativework/category> <http://www.bbc.co.uk/category/sport> .

<http://ex/cwE> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cwE> <http://www.bbc.co.uk/ontologies/creativework/title> "Echo update" .
<http://ex/cwE> <http://www.bbc.co.uk/ontologies/creativework/dateModified> "2011-12-01T07:00:00.000+00:00"^^<http://www.w3.org/2001/XMLSchema#dateTime> .
<http://ex/cwE> <http://www.bbc.co.uk/ontologies/creativework/liveCoverage> "true"^^<http://www.w3.org/2001/XMLSchema#boolean> .
<http://ex/cwE> <http://www.bbc.co.uk/ontologies/creativework/category> <http://www.bbc.co.uk/category/sport> .
<http://ex/cwE> <http://www.bbc.co.uk/ontologies/creativework/audience> <http://www.bbc.co.uk/audience/national> .
"#;

    /// Titles in result order (no sort — verifies ORDER BY DESC is applied).
    fn ordered_titles(g: &GraphSnapshot, works: &[u32]) -> Vec<String> {
        works.iter().map(|&w| pstr(g, w, "title").unwrap_or("?").to_string()).collect()
    }

    #[test]
    fn in_range_newest_first() {
        let g = load_str(FIXTURE).0;
        // 2012 window: cwA(02), cwB(08) bound & in range; cwD lacks audience,
        // cwC is 2013, cwE is 2011. DESC -> Bravo (Aug) before Alpha (Feb).
        let hits = run(&g, "CreativeWork", "2012-01-01", "2012-12-31", 10);
        assert_eq!(ordered_titles(&g, &hits), ["Bravo update", "Alpha update"]);
    }

    #[test]
    fn limit_truncates_after_ordering() {
        let g = load_str(FIXTURE).0;
        // Full range, all bound: cwC(2013), cwB(08/2012), cwA(02/2012), cwE(2011).
        // DESC then LIMIT 2 -> the two newest.
        let hits = run(&g, "CreativeWork", "2010-01-01", "2014-12-31", 2);
        assert_eq!(ordered_titles(&g, &hits), ["Charlie update", "Bravo update"]);
    }

    #[test]
    fn unbound_audience_is_excluded() {
        let g = load_str(FIXTURE).0;
        // cwD (no audience) sits inside the window but must never appear.
        let hits = run(&g, "CreativeWork", "2012-04-01", "2012-06-01", 10);
        assert!(hits.is_empty());
    }

    #[test]
    fn limit_zero_is_empty() {
        let g = load_str(FIXTURE).0;
        assert!(run(&g, "CreativeWork", "2010-01-01", "2014-12-31", 0).is_empty());
    }

    #[test]
    fn unknown_type_is_empty() {
        let g = load_str(FIXTURE).0;
        assert!(run(&g, "NoSuchType", "2000-01-01", "2099-12-31", 10).is_empty());
    }
}

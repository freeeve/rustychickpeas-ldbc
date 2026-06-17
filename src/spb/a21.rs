//! SPB advanced **q21** (faceted search / drill-down): full-text on a word in a
//! creative work's `title`, progressively narrowed by category / audience / tag /
//! liveCoverage / dateCreated facets.
//!
//! Hand translation of advanced `aggregation_standard/query21.txt` (no SPARQL
//! engine). The template's fixed pattern binds a `CreativeWork`'s `title`,
//! `description`, `category`, `tag`, `audience`, `liveCoverage`, `primaryFormat`
//! and `dateCreated`, then layers three substitution FILTERs (`{{{filter1..3}}}`)
//! over a `LIMIT {{{randomLimit}}}`; the drill-down adds one constraint per
//! iteration (FTS word + category, then date, then tag, then format, then a
//! specific day/month/year). We model the search as the title FTS set intersected
//! with the bound facet predicates — the same `fts` core as q8, plus the q3-style
//! `tag` (about/mentions) traversal.
//!
//! Facets (each an `Option`, applied only when bound — `None` = "any"):
//!   * `category_uri` / `audience_uri` — an outgoing `category` / `audience` edge
//!     to a node carrying that `uri`.
//!   * `tag_uri` — an outgoing `about` OR `mentions` edge to that `uri` (the BBC
//!     `cwork:tag` materializes as those two concrete links).
//!   * `live_coverage` — the `liveCoverage` boolean property.
//!   * `date_from` / `date_to` — inclusive lexicographic bounds on the ISO-8601
//!     `dateCreated` string.
//!
//! `{{{projection}}}` / `{{{groupBy}}}` / `{{{orderBy}}}` are runtime drill-down
//! substitution params (each iteration regroups/reorders), so — like `q6_geo` —
//! we return the deduped match set sorted by id, truncated to `limit` (the
//! template's `LIMIT {{{randomLimit}}}`).

use rustychickpeas_core::{Direction, GraphSnapshot};

use crate::props::{pbool, pstr};

/// Whether `work` satisfies every bound facet (an unbound `None` facet matches
/// anything). Mirrors the SPARQL's conjunction of `{{{filter1..3}}}`.
#[allow(clippy::too_many_arguments)]
fn matches_facets(
    g: &GraphSnapshot,
    work: u32,
    category_uri: Option<&str>,
    audience_uri: Option<&str>,
    tag_uri: Option<&str>,
    live_coverage: Option<bool>,
    date_from: Option<&str>,
    date_to: Option<&str>,
) -> bool {
    // SPB q21's fixed BGP requires `cwork:tag ?tag`. The generator emits no literal
    // `cwork:tag`, so — as the doc above and `q5` describe — the topic tag is the
    // `about`/`mentions` link: a solution must carry at least one.
    if g.neighbors_by_type(work, Direction::Outgoing, "about").next().is_none()
        && g.neighbors_by_type(work, Direction::Outgoing, "mentions").next().is_none()
    {
        return false;
    }
    if let Some(uri) = category_uri {
        if !g.has_neighbor_with_property(work, Direction::Outgoing, "category", "uri", uri) {
            return false;
        }
    }
    if let Some(uri) = audience_uri {
        if !g.has_neighbor_with_property(work, Direction::Outgoing, "audience", "uri", uri) {
            return false;
        }
    }
    if let Some(uri) = tag_uri {
        if !(g.has_neighbor_with_property(work, Direction::Outgoing, "about", "uri", uri)
            || g.has_neighbor_with_property(work, Direction::Outgoing, "mentions", "uri", uri))
        {
            return false;
        }
    }
    if let Some(want) = live_coverage {
        if pbool(g, work, "liveCoverage") != want {
            return false;
        }
    }
    if date_from.is_some() || date_to.is_some() {
        let Some(created) = pstr(g, work, "dateCreated") else {
            return false;
        };
        if date_from.is_some_and(|lo| created < lo) || date_to.is_some_and(|hi| created > hi) {
            return false;
        }
    }
    true
}

/// Faceted search: creative works whose `title` matches `word` (full-text) and
/// that satisfy every bound facet, deduped, sorted by id, truncated to `limit`.
#[allow(clippy::too_many_arguments)]
pub fn run(
    g: &GraphSnapshot,
    word: &str,
    category_uri: Option<&str>,
    audience_uri: Option<&str>,
    tag_uri: Option<&str>,
    live_coverage: Option<bool>,
    date_from: Option<&str>,
    date_to: Option<&str>,
    limit: usize,
) -> Vec<u32> {
    let mut out: Vec<u32> = g
        .fts("CreativeWork", "title", word)
        .iter()
        .filter(|&w| {
            matches_facets(g, w, category_uri, audience_uri, tag_uri, live_coverage, date_from, date_to)
        })
        .collect();
    out.sort_unstable();
    out.truncate(limit);
    out
}

#[cfg(test)]
mod tests {
    use super::super::loader::load_str;
    use super::super::queries::name_of;
    use super::*;

    const SPORT: &str = "http://www.bbc.co.uk/category/Sport";
    const NATIONAL: &str = "http://www.bbc.co.uk/audience/National";
    const LONDON: &str = "http://sws.geonames.org/london";

    // Four works; two have "football" in the title (cw-derby, cw-psg). They vary
    // in audience, mentioned tag, liveCoverage and dateCreated so each facet can
    // be exercised independently.
    const FIXTURE: &str = r#"
<http://ex/cw-derby> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw-derby> <http://www.bbc.co.uk/ontologies/creativework/title> "London football derby" .
<http://ex/cw-derby> <http://www.bbc.co.uk/ontologies/creativework/category> <http://www.bbc.co.uk/category/Sport> .
<http://ex/cw-derby> <http://www.bbc.co.uk/ontologies/creativework/audience> <http://www.bbc.co.uk/audience/National> .
<http://ex/cw-derby> <http://www.bbc.co.uk/ontologies/creativework/mentions> <http://sws.geonames.org/london> .
<http://ex/cw-derby> <http://www.bbc.co.uk/ontologies/creativework/liveCoverage> "true"^^<http://www.w3.org/2001/XMLSchema#boolean> .
<http://ex/cw-derby> <http://www.bbc.co.uk/ontologies/creativework/dateCreated> "2024-03-05T09:30:00.000Z"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/cw-psg> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw-psg> <http://www.bbc.co.uk/ontologies/creativework/title> "Paris football match" .
<http://ex/cw-psg> <http://www.bbc.co.uk/ontologies/creativework/category> <http://www.bbc.co.uk/category/Sport> .
<http://ex/cw-psg> <http://www.bbc.co.uk/ontologies/creativework/audience> <http://www.bbc.co.uk/audience/International> .
<http://ex/cw-psg> <http://www.bbc.co.uk/ontologies/creativework/mentions> <http://sws.geonames.org/paris> .
<http://ex/cw-psg> <http://www.bbc.co.uk/ontologies/creativework/liveCoverage> "false"^^<http://www.w3.org/2001/XMLSchema#boolean> .
<http://ex/cw-psg> <http://www.bbc.co.uk/ontologies/creativework/dateCreated> "2024-01-10T08:00:00.000Z"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/cw-tennis> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw-tennis> <http://www.bbc.co.uk/ontologies/creativework/title> "London tennis open" .
<http://ex/cw-tennis> <http://www.bbc.co.uk/ontologies/creativework/category> <http://www.bbc.co.uk/category/Sport> .
<http://ex/cw-tennis> <http://www.bbc.co.uk/ontologies/creativework/audience> <http://www.bbc.co.uk/audience/National> .
<http://ex/cw-tennis> <http://www.bbc.co.uk/ontologies/creativework/mentions> <http://sws.geonames.org/london> .
<http://ex/cw-tennis> <http://www.bbc.co.uk/ontologies/creativework/liveCoverage> "true"^^<http://www.w3.org/2001/XMLSchema#boolean> .
<http://ex/cw-tennis> <http://www.bbc.co.uk/ontologies/creativework/dateCreated> "2024-06-20T18:00:00.000Z"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/cw-cook> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw-cook> <http://www.bbc.co.uk/ontologies/creativework/title> "Cooking show" .
<http://ex/cw-cook> <http://www.bbc.co.uk/ontologies/creativework/category> <http://www.bbc.co.uk/category/Lifestyle> .
<http://ex/cw-cook> <http://www.bbc.co.uk/ontologies/creativework/audience> <http://www.bbc.co.uk/audience/National> .
<http://ex/cw-cook> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/Cooking> .
<http://ex/cw-cook> <http://www.bbc.co.uk/ontologies/creativework/liveCoverage> "false"^^<http://www.w3.org/2001/XMLSchema#boolean> .
<http://ex/cw-cook> <http://www.bbc.co.uk/ontologies/creativework/dateCreated> "2024-02-02T10:00:00.000Z"^^<http://www.w3.org/2001/XMLSchema#dateTime> .
"#;

    fn titles(g: &GraphSnapshot, works: &[u32]) -> Vec<String> {
        let mut t: Vec<String> = works.iter().map(|&w| name_of(g, w).to_string()).collect();
        t.sort();
        t
    }

    fn g() -> GraphSnapshot {
        load_str(FIXTURE).0
    }

    #[test]
    fn fts_only_matches_titles() {
        let g = g();
        // 'football' in title -> the two football works, not tennis/cooking.
        let out = run(&g, "football", None, None, None, None, None, None, 100);
        assert_eq!(titles(&g, &out), ["London football derby", "Paris football match"]);
    }

    #[test]
    fn audience_facet_narrows() {
        let g = g();
        // football + audience=National -> only the derby (PSG is International).
        let out = run(&g, "football", Some(SPORT), Some(NATIONAL), None, None, None, None, 100);
        assert_eq!(titles(&g, &out), ["London football derby"]);
    }

    #[test]
    fn live_coverage_and_tag_facets() {
        let g = g();
        // football + liveCoverage=true -> derby (PSG is false).
        let live = run(&g, "football", None, None, None, Some(true), None, None, 100);
        assert_eq!(titles(&g, &live), ["London football derby"]);
        // football + tag mentions London -> derby (PSG mentions Paris).
        let tagged = run(&g, "football", None, None, Some(LONDON), None, None, None, 100);
        assert_eq!(titles(&g, &tagged), ["London football derby"]);
    }

    #[test]
    fn date_range_facet() {
        let g = g();
        // football created on/after 2024-02-01 -> derby (Mar); PSG (Jan) excluded.
        let out = run(&g, "football", None, None, None, None, Some("2024-02-01"), None, 100);
        assert_eq!(titles(&g, &out), ["London football derby"]);
    }

    #[test]
    fn limit_truncates_after_id_sort() {
        let g = g();
        let out = run(&g, "football", None, None, None, None, None, None, 1);
        assert_eq!(out.len(), 1);
        // Lowest id wins the truncation; cw-derby is declared before cw-psg.
        assert_eq!(titles(&g, &out), ["London football derby"]);
    }

    #[test]
    fn unmatched_facet_yields_empty() {
        let g = g();
        // football but require a category nobody has -> empty.
        let out = run(&g, "football", Some("http://www.bbc.co.uk/category/Weather"), None, None, None, None, None, 100);
        assert!(out.is_empty());
    }
}

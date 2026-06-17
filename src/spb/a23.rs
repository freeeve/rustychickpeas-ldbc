//! SPB advanced **q23** (faceted search drill-down) — the *final* drill-down
//! iteration: per topic tag, the number of distinct creation days.
//!
//! Hand translation of advanced `aggregation_standard/query23.txt` (no SPARQL
//! engine). The template's fixed BGP binds a `CreativeWork`'s `title`,
//! `description`, `category`, `tag`, `audience`, `liveCoverage`, `primaryFormat`
//! and `dateCreated`, decomposes the date, and layers two substitution facets:
//!
//! ```sparql
//! SELECT {{{projection}}} {
//!   ?creativework a cwork:CreativeWork ;
//!     cwork:title ?title ; cwork:description ?description ; cwork:category ?category ;
//!     cwork:tag ?tag ; cwork:audience ?audience ; cwork:liveCoverage ?liveCoverage ;
//!     cwork:primaryFormat ?primaryFormat ; cwork:dateCreated ?dateCreated .
//!   BIND (day(?dateCreated) as ?day) BIND (year(?dateCreated) as ?year) BIND (month(?dateCreated) as ?month)
//!   {{{filter1}}} {{{filter2}}}
//! } {{{groupBy}}} {{{orderBy}}} LIMIT 500
//! ```
//!
//! The query header describes a faceted search adding constraints across
//! iterations: an FTS word in the works' `title`, a `category` constraint, then
//! regrouping, and *finally* "returning days and count of tags grouped by tag".
//! We model that final iteration:
//!   * `{{{filter1}}}` = the FTS word on `title` (core inverted index);
//!   * `{{{filter2}}}` = a `category` facet pinned to a target uri;
//!   * `{{{groupBy}}}` = `GROUP BY ?tag`;
//!   * `{{{projection}}}` = `(?tag, COUNT(DISTINCT (?year,?month,?day)))`;
//!   * `{{{orderBy}}}` = `ORDER BY DESC(count)` (tie-broken by `?tag` ascending).
//!
//! `BIND(day/month/year)` decomposes `dateCreated`; a distinct `(year,month,day)`
//! is exactly a distinct calendar day, so we key each tag's day set on
//! [`parse_date`]'s epoch-day count.
//!
//! Tag / category modeling: the SPB generator emits no literal `cwork:tag`, and
//! `cwork:category` is a uri edge (`bbc/category/...`), not a literal. So — as in
//! q21/q22 — the `category` facet is an outgoing edge to the pinned uri, and the
//! required `cwork:tag ?tag` grouping is folded to the `about`/`mentions` topic
//! links the data actually carries; each grouped "tag" is such a topic, keyed by
//! its uri. The remaining required BGP patterns (`description`, `audience`,
//! `liveCoverage`, `primaryFormat`, `dateCreated`) must be present for a solution.

use std::collections::{HashMap, HashSet};

use rustychickpeas_core::{Direction, GraphSnapshot};

use crate::props::{parse_date, pstr, top_k_by_key};

/// Whether `work` has an outgoing `edge` to a node whose `uri` equals `uri`.
fn has_edge_to_uri(g: &GraphSnapshot, work: u32, edge: &str, uri: &str) -> bool {
    g.neighbors_by_type(work, Direction::Outgoing, edge).any(|t| pstr(g, t, "uri") == Some(uri))
}

/// Whether `work` has at least one outgoing `edge`.
fn has_any_edge(g: &GraphSnapshot, work: u32, edge: &str) -> bool {
    g.neighbors_by_type(work, Direction::Outgoing, edge).next().is_some()
}

/// SPB advanced **q23** final drill-down: over creative works whose `title`
/// matches the full-text `word`, that are `category`-linked to `category_uri`, and
/// that carry the rest of the required BGP (`description`, `audience`,
/// `liveCoverage`, `primaryFormat`, `dateCreated`), count — per topic the work is
/// tagged with (an `about`/`mentions` target) — the number of distinct
/// `dateCreated` calendar days. Returned as `(topic_uri, distinct_days)` ordered
/// by day count descending (tie-broken by uri ascending) and truncated to `limit`
/// (the template's `LIMIT 500`).
pub fn run(g: &GraphSnapshot, word: &str, category_uri: &str, limit: usize) -> Vec<(String, usize)> {
    // topic node -> the set of distinct creation days (epoch-day count) seen for it.
    let mut by_tag: HashMap<u32, HashSet<i64>> = HashMap::new();
    for w in g.fts("CreativeWork", "title", word).iter() {
        // {{{filter2}}}: the pinned category facet (an outgoing edge to the uri).
        if !has_edge_to_uri(g, w, "category", category_uri) {
            continue;
        }
        // The rest of the fixed BGP must be bound for a solution to exist.
        let Some(created) = g.str_prop(w, "dateCreated") else {
            continue;
        };
        if g.str_prop(w, "description").is_none()
            || !has_any_edge(g, w, "audience")
            || !has_any_edge(g, w, "primaryFormat")
            || g.prop(w, "liveCoverage").is_none()
        {
            continue;
        }
        // BIND(day/month/year): a distinct (year,month,day) is a distinct epoch day.
        let Some((_, day)) = parse_date(created) else {
            continue;
        };
        // The required `cwork:tag ?tag` grouping, folded to about/mentions topics.
        for t in g
            .neighbors_by_type(w, Direction::Outgoing, "about")
            .chain(g.neighbors_by_type(w, Direction::Outgoing, "mentions"))
        {
            by_tag.entry(t).or_default().insert(day);
        }
    }

    let rows =
        by_tag.into_iter().map(|(t, days)| (pstr(g, t, "uri").unwrap_or("?").to_string(), days.len()));
    top_k_by_key(rows, limit)
}

#[cfg(test)]
mod tests {
    use super::super::loader::load_str;
    use super::*;

    const SPORT: &str = "http://www.bbc.co.uk/category/sport";
    const POLITICS: &str = "http://www.bbc.co.uk/category/politics";
    const TEAM_X: &str = "http://dbpedia.org/resource/TeamX";
    const TEAM_Y: &str = "http://dbpedia.org/resource/TeamY";
    const TEAM_A: &str = "http://dbpedia.org/resource/TeamA";
    const TEAM_Z: &str = "http://dbpedia.org/resource/TeamZ";

    // Eight creative works. Most match "football" in the title and are
    // category-linked to "sport"; the topic each is `about` and its dateCreated
    // vary so per-topic distinct-day counts, DISTINCT dedup, the count-desc /
    // uri-asc ordering, and the BGP/category/FTS exclusions can each be checked.
    //   cw1: about TeamX, sport, 2024-01-01
    //   cw2: about TeamX, sport, 2024-01-02
    //   cw3: about TeamX, sport, 2024-01-02  (same day as cw2 -> DISTINCT keeps TeamX at 2)
    //   cw4: about TeamY, sport, 2024-03-05
    //   cw5: about TeamZ, POLITICS, 2024-04-04  (excluded when category=sport)
    //   cw6: about TeamY, sport, TENNIS title   (excluded by the football FTS)
    //   cw7: about TeamW, sport, 2024-06-06, NO primaryFormat (excluded by the BGP)
    //   cw8: about TeamA, sport, 2024-05-05     (ties TeamY's count of 1 -> uri-asc tie-break)
    const FIXTURE: &str = r#"
<http://ex/cw1> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw1> <http://www.bbc.co.uk/ontologies/creativework/title> "London football derby" .
<http://ex/cw1> <http://www.bbc.co.uk/ontologies/creativework/description> "a match report" .
<http://ex/cw1> <http://www.bbc.co.uk/ontologies/creativework/category> <http://www.bbc.co.uk/category/sport> .
<http://ex/cw1> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/TeamX> .
<http://ex/cw1> <http://www.bbc.co.uk/ontologies/creativework/audience> <http://www.bbc.co.uk/audience/national> .
<http://ex/cw1> <http://www.bbc.co.uk/ontologies/creativework/liveCoverage> "true"^^<http://www.w3.org/2001/XMLSchema#boolean> .
<http://ex/cw1> <http://www.bbc.co.uk/ontologies/creativework/primaryFormat> <http://www.bbc.co.uk/format/textual> .
<http://ex/cw1> <http://www.bbc.co.uk/ontologies/creativework/dateCreated> "2024-01-01T12:00:00.000+00:00"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/cw2> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw2> <http://www.bbc.co.uk/ontologies/creativework/title> "Paris football club" .
<http://ex/cw2> <http://www.bbc.co.uk/ontologies/creativework/description> "a transfer story" .
<http://ex/cw2> <http://www.bbc.co.uk/ontologies/creativework/category> <http://www.bbc.co.uk/category/sport> .
<http://ex/cw2> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/TeamX> .
<http://ex/cw2> <http://www.bbc.co.uk/ontologies/creativework/audience> <http://www.bbc.co.uk/audience/national> .
<http://ex/cw2> <http://www.bbc.co.uk/ontologies/creativework/liveCoverage> "false"^^<http://www.w3.org/2001/XMLSchema#boolean> .
<http://ex/cw2> <http://www.bbc.co.uk/ontologies/creativework/primaryFormat> <http://www.bbc.co.uk/format/video> .
<http://ex/cw2> <http://www.bbc.co.uk/ontologies/creativework/dateCreated> "2024-01-02T08:00:00.000+00:00"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/cw3> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw3> <http://www.bbc.co.uk/ontologies/creativework/title> "Berlin football match" .
<http://ex/cw3> <http://www.bbc.co.uk/ontologies/creativework/description> "a fixture note" .
<http://ex/cw3> <http://www.bbc.co.uk/ontologies/creativework/category> <http://www.bbc.co.uk/category/sport> .
<http://ex/cw3> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/TeamX> .
<http://ex/cw3> <http://www.bbc.co.uk/ontologies/creativework/audience> <http://www.bbc.co.uk/audience/national> .
<http://ex/cw3> <http://www.bbc.co.uk/ontologies/creativework/liveCoverage> "true"^^<http://www.w3.org/2001/XMLSchema#boolean> .
<http://ex/cw3> <http://www.bbc.co.uk/ontologies/creativework/primaryFormat> <http://www.bbc.co.uk/format/textual> .
<http://ex/cw3> <http://www.bbc.co.uk/ontologies/creativework/dateCreated> "2024-01-02T19:30:00.000+00:00"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/cw4> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw4> <http://www.bbc.co.uk/ontologies/creativework/title> "Rome football news" .
<http://ex/cw4> <http://www.bbc.co.uk/ontologies/creativework/description> "a news brief" .
<http://ex/cw4> <http://www.bbc.co.uk/ontologies/creativework/category> <http://www.bbc.co.uk/category/sport> .
<http://ex/cw4> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/TeamY> .
<http://ex/cw4> <http://www.bbc.co.uk/ontologies/creativework/audience> <http://www.bbc.co.uk/audience/national> .
<http://ex/cw4> <http://www.bbc.co.uk/ontologies/creativework/liveCoverage> "false"^^<http://www.w3.org/2001/XMLSchema#boolean> .
<http://ex/cw4> <http://www.bbc.co.uk/ontologies/creativework/primaryFormat> <http://www.bbc.co.uk/format/textual> .
<http://ex/cw4> <http://www.bbc.co.uk/ontologies/creativework/dateCreated> "2024-03-05T10:00:00.000+00:00"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/cw5> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw5> <http://www.bbc.co.uk/ontologies/creativework/title> "Madrid football report" .
<http://ex/cw5> <http://www.bbc.co.uk/ontologies/creativework/description> "a derby preview" .
<http://ex/cw5> <http://www.bbc.co.uk/ontologies/creativework/category> <http://www.bbc.co.uk/category/politics> .
<http://ex/cw5> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/TeamZ> .
<http://ex/cw5> <http://www.bbc.co.uk/ontologies/creativework/audience> <http://www.bbc.co.uk/audience/national> .
<http://ex/cw5> <http://www.bbc.co.uk/ontologies/creativework/liveCoverage> "false"^^<http://www.w3.org/2001/XMLSchema#boolean> .
<http://ex/cw5> <http://www.bbc.co.uk/ontologies/creativework/primaryFormat> <http://www.bbc.co.uk/format/textual> .
<http://ex/cw5> <http://www.bbc.co.uk/ontologies/creativework/dateCreated> "2024-04-04T11:00:00.000+00:00"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/cw6> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw6> <http://www.bbc.co.uk/ontologies/creativework/title> "Wimbledon tennis final" .
<http://ex/cw6> <http://www.bbc.co.uk/ontologies/creativework/description> "a championship preview" .
<http://ex/cw6> <http://www.bbc.co.uk/ontologies/creativework/category> <http://www.bbc.co.uk/category/sport> .
<http://ex/cw6> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/TeamY> .
<http://ex/cw6> <http://www.bbc.co.uk/ontologies/creativework/audience> <http://www.bbc.co.uk/audience/national> .
<http://ex/cw6> <http://www.bbc.co.uk/ontologies/creativework/liveCoverage> "true"^^<http://www.w3.org/2001/XMLSchema#boolean> .
<http://ex/cw6> <http://www.bbc.co.uk/ontologies/creativework/primaryFormat> <http://www.bbc.co.uk/format/textual> .
<http://ex/cw6> <http://www.bbc.co.uk/ontologies/creativework/dateCreated> "2024-02-02T08:00:00.000+00:00"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/cw7> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw7> <http://www.bbc.co.uk/ontologies/creativework/title> "Lyon football digest" .
<http://ex/cw7> <http://www.bbc.co.uk/ontologies/creativework/description> "a weekly digest" .
<http://ex/cw7> <http://www.bbc.co.uk/ontologies/creativework/category> <http://www.bbc.co.uk/category/sport> .
<http://ex/cw7> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/TeamW> .
<http://ex/cw7> <http://www.bbc.co.uk/ontologies/creativework/audience> <http://www.bbc.co.uk/audience/national> .
<http://ex/cw7> <http://www.bbc.co.uk/ontologies/creativework/liveCoverage> "false"^^<http://www.w3.org/2001/XMLSchema#boolean> .
<http://ex/cw7> <http://www.bbc.co.uk/ontologies/creativework/dateCreated> "2024-06-06T07:00:00.000+00:00"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/cw8> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw8> <http://www.bbc.co.uk/ontologies/creativework/title> "Turin football extra" .
<http://ex/cw8> <http://www.bbc.co.uk/ontologies/creativework/description> "extra coverage" .
<http://ex/cw8> <http://www.bbc.co.uk/ontologies/creativework/category> <http://www.bbc.co.uk/category/sport> .
<http://ex/cw8> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/TeamA> .
<http://ex/cw8> <http://www.bbc.co.uk/ontologies/creativework/audience> <http://www.bbc.co.uk/audience/national> .
<http://ex/cw8> <http://www.bbc.co.uk/ontologies/creativework/liveCoverage> "true"^^<http://www.w3.org/2001/XMLSchema#boolean> .
<http://ex/cw8> <http://www.bbc.co.uk/ontologies/creativework/primaryFormat> <http://www.bbc.co.uk/format/textual> .
<http://ex/cw8> <http://www.bbc.co.uk/ontologies/creativework/dateCreated> "2024-05-05T09:00:00.000+00:00"^^<http://www.w3.org/2001/XMLSchema#dateTime> .
"#;

    fn g() -> GraphSnapshot {
        load_str(FIXTURE).0
    }

    #[test]
    fn per_topic_distinct_day_counts_ordered() {
        let g = g();
        // football + category=sport: TeamX has two distinct days (cw1 01-01, and
        // cw2/cw3 sharing 01-02 -> DISTINCT keeps it at 2); TeamY and TeamA have
        // one day each. cw5 (politics), cw6 (tennis) and cw7 (no primaryFormat)
        // are excluded. Order: count desc, then uri asc (TeamA before TeamY).
        let rows = run(&g, "football", SPORT, 500);
        assert_eq!(
            rows,
            vec![(TEAM_X.to_string(), 2), (TEAM_A.to_string(), 1), (TEAM_Y.to_string(), 1)]
        );
    }

    #[test]
    fn limit_truncates_after_ordering() {
        let g = g();
        // The highest-count topic survives a LIMIT 1.
        assert_eq!(run(&g, "football", SPORT, 1), vec![(TEAM_X.to_string(), 2)]);
    }

    #[test]
    fn category_pin_selects_its_own_works() {
        let g = g();
        // Pinning category=politics leaves only cw5 -> TeamZ with its single day.
        assert_eq!(run(&g, "football", POLITICS, 500), vec![(TEAM_Z.to_string(), 1)]);
    }

    #[test]
    fn fts_miss_yields_empty() {
        let g = g();
        assert!(run(&g, "cricket", SPORT, 500).is_empty());
    }
}

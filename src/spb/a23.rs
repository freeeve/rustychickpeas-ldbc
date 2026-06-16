//! SPB advanced **q23** (faceted search drill-down) — the *final* drill-down
//! iteration: per `tag`, the number of distinct creation-days.
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
//! The query header describes a faceted search that adds constraints across
//! iterations: an FTS word in the works' `title`, then a `category` constraint,
//! regrouping by year/month, then by tag, then by primary format, and *finally*
//! "returning days and count of tags grouped by tag". We model that final
//! iteration:
//!   * `{{{filter1}}}` = the FTS word on `title` (core inverted index);
//!   * `{{{filter2}}}` = the `category` value pin (`?category = category`);
//!   * `{{{groupBy}}}` = `GROUP BY ?tag`;
//!   * `{{{projection}}}` = `(?tag, COUNT(DISTINCT (?year,?month,?day)))`;
//!   * `{{{orderBy}}}` = `ORDER BY DESC(count)` (tie-broken by `?tag` ascending).
//!
//! `BIND(day/month/year)` decomposes `dateCreated`; a distinct `(year,month,day)`
//! is exactly a distinct calendar day, so we key the per-tag day set on
//! [`parse_date`]'s epoch-day count.
//!
//! Caveats / deviations:
//! - Unlike q21/q22 — which materialize `category` / `tag` / `audience` /
//!   `primaryFormat` as out-edges matched by a target `uri` — q23 must *group by
//!   the tag value* and *pin a category value*, so we read them as the literal
//!   string properties the q23 vocabulary lists (`pstr`), not as edges.
//! - The loader keeps only the **first** literal for a `(node, predicate)` pair
//!   (`load_str` dedups properties), so `tag` / `category` are effectively
//!   single-valued here. SPB's `cwork:tag` can in principle repeat (a work would
//!   then count toward several tags); with the literal-property loader only the
//!   first survives. Treated as single-valued; deviation noted.
//! - A dense string column reads back as `Some("")` for a node lacking the
//!   property, so every required facet prop is read through `req_str`, which
//!   treats empty as absent — a row only exists when the whole BGP is bound.
//! - `title` is required by the BGP but is already implied by the `title` FTS
//!   hit, so we do not re-check it (same reasoning as q22).

use std::collections::{HashMap, HashSet};

use rustychickpeas_core::GraphSnapshot;

use crate::props::{parse_date, pstr};

/// Read a required dense string property, treating the empty-string sentinel a
/// dense column returns for an absent value as "not present".
fn req_str<'a>(g: &'a GraphSnapshot, node: u32, prop: &str) -> Option<&'a str> {
    pstr(g, node, prop).filter(|s| !s.is_empty())
}

/// SPB advanced **q23** final drill-down: over creative works whose `title`
/// matches the full-text `word` and whose `category` equals `category` (and that
/// carry the rest of the required BGP — `description`, `tag`, `audience`,
/// `liveCoverage`, `primaryFormat`, `dateCreated`), count, per distinct `tag`
/// value, the number of distinct `dateCreated` calendar days. Returned as
/// `(tag, distinct_days)` ordered by day count descending (tie-broken by tag
/// ascending) and truncated to `limit` (the template's `LIMIT 500`).
pub fn run(g: &GraphSnapshot, word: &str, category: &str, limit: usize) -> Vec<(String, usize)> {
    // tag value -> the set of distinct creation days (epoch-day count) seen for it.
    let mut by_tag: HashMap<&str, HashSet<i64>> = HashMap::new();
    for w in g.fts("CreativeWork", "title", word).iter() {
        // {{{filter2}}}: the pinned category value.
        if req_str(g, w, "category") != Some(category) {
            continue;
        }
        // The rest of the BGP must be bound for a solution to exist; `tag` is the
        // grouping key and `dateCreated` carries the day decomposition.
        let (Some(tag), Some(created)) = (req_str(g, w, "tag"), req_str(g, w, "dateCreated")) else {
            continue;
        };
        if req_str(g, w, "description").is_none()
            || req_str(g, w, "audience").is_none()
            || req_str(g, w, "primaryFormat").is_none()
            || g.prop(w, "liveCoverage").is_none()
        {
            continue;
        }
        // BIND(day/month/year): a distinct (year,month,day) is a distinct epoch day.
        let Some((_, day)) = parse_date(created) else {
            continue;
        };
        by_tag.entry(tag).or_default().insert(day);
    }

    let mut rows: Vec<(String, usize)> =
        by_tag.into_iter().map(|(tag, days)| (tag.to_string(), days.len())).collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    rows.truncate(limit);
    rows
}

#[cfg(test)]
mod tests {
    use super::super::loader::load_str;
    use super::*;

    // Eight creative works. Most match "football" in the title and sit in the
    // "sport" category; tags and dateCreated vary so per-tag distinct-day counts,
    // DISTINCT dedup, the count-desc / tag-asc ordering, and the BGP/category/FTS
    // exclusions can each be checked.
    //   cw1: teamX, sport, 2024-01-01
    //   cw2: teamX, sport, 2024-01-02
    //   cw3: teamX, sport, 2024-01-02  (same day as cw2 -> DISTINCT keeps teamX at 2)
    //   cw4: teamY, sport, 2024-03-05
    //   cw5: teamZ, POLITICS, 2024-04-04  (excluded when category=sport)
    //   cw6: teamY, sport, TENNIS title   (excluded by the football FTS)
    //   cw7: teamW, sport, 2024-06-06, NO primaryFormat (excluded by the BGP)
    //   cw8: teamA, sport, 2024-05-05     (ties teamY's count of 1 -> tag-asc tie-break)
    const FIXTURE: &str = r#"
<http://ex/cw1> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw1> <http://www.bbc.co.uk/ontologies/creativework/title> "London football derby" .
<http://ex/cw1> <http://www.bbc.co.uk/ontologies/creativework/description> "a match report" .
<http://ex/cw1> <http://www.bbc.co.uk/ontologies/creativework/category> "sport" .
<http://ex/cw1> <http://www.bbc.co.uk/ontologies/creativework/tag> "teamX" .
<http://ex/cw1> <http://www.bbc.co.uk/ontologies/creativework/audience> "national" .
<http://ex/cw1> <http://www.bbc.co.uk/ontologies/creativework/liveCoverage> "true"^^<http://www.w3.org/2001/XMLSchema#boolean> .
<http://ex/cw1> <http://www.bbc.co.uk/ontologies/creativework/primaryFormat> "textual" .
<http://ex/cw1> <http://www.bbc.co.uk/ontologies/creativework/dateCreated> "2024-01-01T12:00:00.000+00:00"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/cw2> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw2> <http://www.bbc.co.uk/ontologies/creativework/title> "Paris football club" .
<http://ex/cw2> <http://www.bbc.co.uk/ontologies/creativework/description> "a transfer story" .
<http://ex/cw2> <http://www.bbc.co.uk/ontologies/creativework/category> "sport" .
<http://ex/cw2> <http://www.bbc.co.uk/ontologies/creativework/tag> "teamX" .
<http://ex/cw2> <http://www.bbc.co.uk/ontologies/creativework/audience> "national" .
<http://ex/cw2> <http://www.bbc.co.uk/ontologies/creativework/liveCoverage> "false"^^<http://www.w3.org/2001/XMLSchema#boolean> .
<http://ex/cw2> <http://www.bbc.co.uk/ontologies/creativework/primaryFormat> "video" .
<http://ex/cw2> <http://www.bbc.co.uk/ontologies/creativework/dateCreated> "2024-01-02T08:00:00.000+00:00"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/cw3> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw3> <http://www.bbc.co.uk/ontologies/creativework/title> "Berlin football match" .
<http://ex/cw3> <http://www.bbc.co.uk/ontologies/creativework/description> "a fixture note" .
<http://ex/cw3> <http://www.bbc.co.uk/ontologies/creativework/category> "sport" .
<http://ex/cw3> <http://www.bbc.co.uk/ontologies/creativework/tag> "teamX" .
<http://ex/cw3> <http://www.bbc.co.uk/ontologies/creativework/audience> "national" .
<http://ex/cw3> <http://www.bbc.co.uk/ontologies/creativework/liveCoverage> "true"^^<http://www.w3.org/2001/XMLSchema#boolean> .
<http://ex/cw3> <http://www.bbc.co.uk/ontologies/creativework/primaryFormat> "textual" .
<http://ex/cw3> <http://www.bbc.co.uk/ontologies/creativework/dateCreated> "2024-01-02T19:30:00.000+00:00"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/cw4> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw4> <http://www.bbc.co.uk/ontologies/creativework/title> "Rome football news" .
<http://ex/cw4> <http://www.bbc.co.uk/ontologies/creativework/description> "a news brief" .
<http://ex/cw4> <http://www.bbc.co.uk/ontologies/creativework/category> "sport" .
<http://ex/cw4> <http://www.bbc.co.uk/ontologies/creativework/tag> "teamY" .
<http://ex/cw4> <http://www.bbc.co.uk/ontologies/creativework/audience> "national" .
<http://ex/cw4> <http://www.bbc.co.uk/ontologies/creativework/liveCoverage> "false"^^<http://www.w3.org/2001/XMLSchema#boolean> .
<http://ex/cw4> <http://www.bbc.co.uk/ontologies/creativework/primaryFormat> "textual" .
<http://ex/cw4> <http://www.bbc.co.uk/ontologies/creativework/dateCreated> "2024-03-05T10:00:00.000+00:00"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/cw5> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw5> <http://www.bbc.co.uk/ontologies/creativework/title> "Madrid football report" .
<http://ex/cw5> <http://www.bbc.co.uk/ontologies/creativework/description> "a derby preview" .
<http://ex/cw5> <http://www.bbc.co.uk/ontologies/creativework/category> "politics" .
<http://ex/cw5> <http://www.bbc.co.uk/ontologies/creativework/tag> "teamZ" .
<http://ex/cw5> <http://www.bbc.co.uk/ontologies/creativework/audience> "national" .
<http://ex/cw5> <http://www.bbc.co.uk/ontologies/creativework/liveCoverage> "false"^^<http://www.w3.org/2001/XMLSchema#boolean> .
<http://ex/cw5> <http://www.bbc.co.uk/ontologies/creativework/primaryFormat> "textual" .
<http://ex/cw5> <http://www.bbc.co.uk/ontologies/creativework/dateCreated> "2024-04-04T11:00:00.000+00:00"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/cw6> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw6> <http://www.bbc.co.uk/ontologies/creativework/title> "Wimbledon tennis final" .
<http://ex/cw6> <http://www.bbc.co.uk/ontologies/creativework/description> "a championship preview" .
<http://ex/cw6> <http://www.bbc.co.uk/ontologies/creativework/category> "sport" .
<http://ex/cw6> <http://www.bbc.co.uk/ontologies/creativework/tag> "teamY" .
<http://ex/cw6> <http://www.bbc.co.uk/ontologies/creativework/audience> "national" .
<http://ex/cw6> <http://www.bbc.co.uk/ontologies/creativework/liveCoverage> "true"^^<http://www.w3.org/2001/XMLSchema#boolean> .
<http://ex/cw6> <http://www.bbc.co.uk/ontologies/creativework/primaryFormat> "textual" .
<http://ex/cw6> <http://www.bbc.co.uk/ontologies/creativework/dateCreated> "2024-02-02T08:00:00.000+00:00"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/cw7> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw7> <http://www.bbc.co.uk/ontologies/creativework/title> "Lyon football digest" .
<http://ex/cw7> <http://www.bbc.co.uk/ontologies/creativework/description> "a weekly digest" .
<http://ex/cw7> <http://www.bbc.co.uk/ontologies/creativework/category> "sport" .
<http://ex/cw7> <http://www.bbc.co.uk/ontologies/creativework/tag> "teamW" .
<http://ex/cw7> <http://www.bbc.co.uk/ontologies/creativework/audience> "national" .
<http://ex/cw7> <http://www.bbc.co.uk/ontologies/creativework/liveCoverage> "false"^^<http://www.w3.org/2001/XMLSchema#boolean> .
<http://ex/cw7> <http://www.bbc.co.uk/ontologies/creativework/dateCreated> "2024-06-06T07:00:00.000+00:00"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/cw8> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw8> <http://www.bbc.co.uk/ontologies/creativework/title> "Turin football extra" .
<http://ex/cw8> <http://www.bbc.co.uk/ontologies/creativework/description> "extra coverage" .
<http://ex/cw8> <http://www.bbc.co.uk/ontologies/creativework/category> "sport" .
<http://ex/cw8> <http://www.bbc.co.uk/ontologies/creativework/tag> "teamA" .
<http://ex/cw8> <http://www.bbc.co.uk/ontologies/creativework/audience> "national" .
<http://ex/cw8> <http://www.bbc.co.uk/ontologies/creativework/liveCoverage> "true"^^<http://www.w3.org/2001/XMLSchema#boolean> .
<http://ex/cw8> <http://www.bbc.co.uk/ontologies/creativework/primaryFormat> "textual" .
<http://ex/cw8> <http://www.bbc.co.uk/ontologies/creativework/dateCreated> "2024-05-05T09:00:00.000+00:00"^^<http://www.w3.org/2001/XMLSchema#dateTime> .
"#;

    fn g() -> GraphSnapshot {
        load_str(FIXTURE).0
    }

    #[test]
    fn per_tag_distinct_day_counts_ordered() {
        let g = g();
        // football + category=sport: teamX has two distinct days (cw1 01-01, and
        // cw2/cw3 sharing 01-02 -> DISTINCT keeps it at 2); teamY and teamA have
        // one day each. cw5 (politics), cw6 (tennis) and cw7 (no primaryFormat)
        // are excluded. Order: count desc, then tag asc (teamA before teamY).
        let rows = run(&g, "football", "sport", 500);
        assert_eq!(
            rows,
            vec![("teamX".to_string(), 2), ("teamA".to_string(), 1), ("teamY".to_string(), 1)]
        );
    }

    #[test]
    fn limit_truncates_after_ordering() {
        let g = g();
        // The highest-count tag survives a LIMIT 1.
        assert_eq!(run(&g, "football", "sport", 1), vec![("teamX".to_string(), 2)]);
    }

    #[test]
    fn category_pin_selects_its_own_works() {
        let g = g();
        // Pinning category=politics leaves only cw5 -> teamZ with its single day.
        assert_eq!(run(&g, "football", "politics", 500), vec![("teamZ".to_string(), 1)]);
    }

    #[test]
    fn fts_miss_yields_empty() {
        let g = g();
        assert!(run(&g, "cricket", "sport", 500).is_empty());
    }
}

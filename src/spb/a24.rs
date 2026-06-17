//! SPB advanced **q24** (relatedness time-line) — "retrieve a time-line of
//! relatedness between two entities: a per-day count of the creative works that
//! tag BOTH entities together, optionally limited to a time interval".
//!
//! Hand translation of `advanced/aggregation_standard/query24.txt` (no SPARQL
//! engine). The query body is:
//!
//! ```sparql
//! SELECT DISTINCT ?year ?month ?day ((COUNT(*)) AS ?cwsPerDay) {
//!   ?cw a cwork:CreativeWork .
//!   ?cw cwork:about {{{entityA}}} .
//!   ?cw cwork:about {{{entityB}}} .
//!   ?cw cwork:dateCreated ?dateCreated .
//!   BIND (day(?dateCreated)   AS ?day) .
//!   BIND (month(?dateCreated) AS ?month) .
//!   BIND (year(?dateCreated)  AS ?year) .
//!   {{{timeFilter}}}
//! } GROUP BY ?year ?month ?day ORDER BY ?year ?month ?day
//! ```
//!
//! The two `cwork:about {{{entityX}}}` patterns make the work set the
//! **intersection** of the two entities' incoming `about` neighbours: we collect
//! `entityA`'s incoming `about` works, then keep those that are also `about`
//! `entityB` and that carry the `CreativeWork` label (`a cwork:CreativeWork`). For
//! a given work each constant-bound `about` pattern yields exactly one solution,
//! so `COUNT(*)` per `(year, month, day)` group is simply the number of distinct
//! qualifying works created that day; a `HashSet` of works supplies the implicit
//! DISTINCT (and absorbs duplicate `about` triples / the `entityA == entityB`
//! degenerate case). `?year`/`?month`/`?day` come from the leading `YYYY-MM-DD` of
//! the required ISO-8601 `cwork:dateCreated`, and a `BTreeMap` keyed by that triple
//! realises both the GROUP BY and the ascending ORDER BY.
//!
//! Caveats / deviations:
//! - `{{{timeFilter}}}` is the template's optional ±6-month date window. We model
//!   it as an inclusive `[from, to]` day range (each `YYYY-MM-DD`, compared against
//!   the work's `dateCreated` date prefix); passing `None`/`None` yields the
//!   unconstrained time-line. The prefix compare (not the full ISO string) keeps a
//!   work created exactly on the `to` day inside the window.
//! - `cwork:dateCreated` is a required pattern; a work lacking it (a dense string
//!   column reads back as `""`) or with an unparseable date is dropped, never
//!   counted under a zeroed day.

use std::collections::{BTreeMap, HashSet};

use rustychickpeas_core::{bitmap::NodeSet, Direction, GraphSnapshot};

use super::queries::node_by_uri;
use crate::props::{parse_ymd, pstr};

/// SPB advanced q24: the per-day count of creative works tagging BOTH `uri_a`
/// and `uri_b` (via `cwork:about`), as `((year, month, day), count)` rows sorted
/// ascending by date. `from` / `to` are the optional inclusive `{{{timeFilter}}}`
/// day bounds (`YYYY-MM-DD`); `None` leaves that side unbounded. An unknown entity
/// uri yields no rows.
pub fn run(
    g: &GraphSnapshot,
    uri_a: &str,
    uri_b: &str,
    from: Option<&str>,
    to: Option<&str>,
) -> Vec<((i32, u32, u32), usize)> {
    let (Some(a), Some(b)) = (node_by_uri(g, uri_a), node_by_uri(g, uri_b)) else {
        return Vec::new();
    };

    // Works `about` entityA, as a node set (the build side of the intersection).
    let mut about_a = NodeSet::empty();
    for w in g.neighbors_by_type(a, Direction::Incoming, "about") {
        about_a.insert(w);
    }

    // Works `about` BOTH entities — entityB's `about` sources that are in
    // `about_a` — kept iff CreativeWorks; the HashSet dedups a work reached twice.
    let mut both: HashSet<u32> = HashSet::new();
    for w in g.neighbors_in_set(b, Direction::Incoming, "about", &about_a) {
        if g.has_label(w, "CreativeWork") {
            both.insert(w);
        }
    }

    // GROUP BY (year, month, day) with COUNT(*); the BTreeMap is the ORDER BY.
    let mut per_day: BTreeMap<(i32, u32, u32), usize> = BTreeMap::new();
    for w in both {
        // `cwork:dateCreated` is required; treat the dense-column "" as absent.
        let Some(created) = pstr(g, w, "dateCreated").filter(|s| !s.is_empty()) else {
            continue;
        };
        let Some(key) = parse_ymd(created) else {
            continue;
        };
        // {{{timeFilter}}}: inclusive [from, to] window on the date prefix.
        let day = &created[..10];
        if from.is_some_and(|lo| day < lo) || to.is_some_and(|hi| day > hi) {
            continue;
        }
        *per_day.entry(key).or_insert(0) += 1;
    }
    per_day.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::super::loader::load_str;
    use super::*;

    const ACME: &str = "http://dbpedia.org/resource/Acme_Corp";
    const SUMMIT: &str = "http://dbpedia.org/resource/Summit_2024";
    const LONELY: &str = "http://dbpedia.org/resource/Lonely_Co";

    // Entity A is a Company, entity B an Event; Lonely_Co is an unrelated Company.
    // cw1+cw2 are `about` BOTH on 2024-06-01 (one day, two works -> count 2);
    // cw3 is `about` BOTH on 2024-07-15; cw4 is `about` BOTH on the earlier
    // 2024-05-20 (to prove ascending order). cw5 is `about` A only and cw6 is
    // `about` B only -- both decoys the intersection must drop.
    const FIXTURE: &str = r#"
<http://dbpedia.org/resource/Acme_Corp> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://dbpedia.org/ontology/Company> .
<http://dbpedia.org/resource/Summit_2024> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://dbpedia.org/ontology/Event> .
<http://dbpedia.org/resource/Lonely_Co> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://dbpedia.org/ontology/Company> .

<http://ex/cw1> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw1> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/Acme_Corp> .
<http://ex/cw1> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/Summit_2024> .
<http://ex/cw1> <http://www.bbc.co.uk/ontologies/creativework/dateCreated> "2024-06-01T08:00:00.000+00:00"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/cw2> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw2> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/Acme_Corp> .
<http://ex/cw2> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/Summit_2024> .
<http://ex/cw2> <http://www.bbc.co.uk/ontologies/creativework/dateCreated> "2024-06-01T19:30:00.000+00:00"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/cw3> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw3> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/Acme_Corp> .
<http://ex/cw3> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/Summit_2024> .
<http://ex/cw3> <http://www.bbc.co.uk/ontologies/creativework/dateCreated> "2024-07-15T12:00:00.000+00:00"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/cw4> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw4> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/Acme_Corp> .
<http://ex/cw4> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/Summit_2024> .
<http://ex/cw4> <http://www.bbc.co.uk/ontologies/creativework/dateCreated> "2024-05-20T10:00:00.000+00:00"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/cw5> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw5> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/Acme_Corp> .
<http://ex/cw5> <http://www.bbc.co.uk/ontologies/creativework/dateCreated> "2024-06-01T06:00:00.000+00:00"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/cw6> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw6> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/Summit_2024> .
<http://ex/cw6> <http://www.bbc.co.uk/ontologies/creativework/dateCreated> "2024-07-15T09:00:00.000+00:00"^^<http://www.w3.org/2001/XMLSchema#dateTime> .
"#;

    fn g() -> GraphSnapshot {
        load_str(FIXTURE).0
    }

    #[test]
    fn groups_per_day_counts_works_about_both_ascending() {
        let g = g();
        // Unconstrained time-line: only works `about` BOTH entities, grouped per
        // day, ascending. cw5 (A only) and cw6 (B only) are excluded; 2024-06-01
        // carries two works (cw1, cw2) so its count is 2.
        assert_eq!(
            run(&g, ACME, SUMMIT, None, None),
            vec![((2024, 5, 20), 1), ((2024, 6, 1), 2), ((2024, 7, 15), 1)]
        );
        // The intersection is symmetric in the two entities.
        assert_eq!(run(&g, SUMMIT, ACME, None, None), run(&g, ACME, SUMMIT, None, None));
    }

    #[test]
    fn time_filter_restricts_to_window() {
        let g = g();
        // A ~6-month window starting 2024-06-01 drops the earlier 2024-05-20 day;
        // the inclusive lower bound keeps the works created on 2024-06-01 itself.
        assert_eq!(
            run(&g, ACME, SUMMIT, Some("2024-06-01"), Some("2024-11-30")),
            vec![((2024, 6, 1), 2), ((2024, 7, 15), 1)]
        );
        // An upper bound on 2024-06-01 keeps just that day despite its time-of-day.
        assert_eq!(
            run(&g, ACME, SUMMIT, None, Some("2024-06-01")),
            vec![((2024, 5, 20), 1), ((2024, 6, 1), 2)]
        );
    }

    #[test]
    fn unknown_or_non_overlapping_entity_is_empty() {
        let g = g();
        // An unknown uri resolves to no node -> no rows.
        assert!(run(&g, ACME, "http://dbpedia.org/resource/Nobody", None, None).is_empty());
        // A known entity sharing no creative work with Acme -> empty intersection.
        assert!(run(&g, ACME, LONELY, None, None).is_empty());
    }
}

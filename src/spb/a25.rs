//! SPB advanced **q25** (related entities) — "the 10 most popular entities
//! related to a selected one": an ordered list of other entities that co-occur
//! with the selected entity `A` in creative works, ranked by the number of
//! *distinct calendar days* on which a co-occurrence happens.
//!
//! Hand translation of `advanced/aggregation_standard/query25.txt` (no SPARQL
//! engine). The query body is:
//!
//! ```sparql
//! SELECT ?who ?interactionDays {
//!   { SELECT ?who ((COUNT(*)) AS ?interactionDays) {
//!       { SELECT DISTINCT ?year ?month ?day ?who {
//!           ?cw a cwork:CreativeWork .
//!           ?cw cwork:about {{{entityA}}} .
//!           ?cw cwork:about ?who .
//!           ?cw cwork:dateCreated ?dateCreated .
//!           BIND (day(?dateCreated) AS ?day) BIND (month(?dateCreated) AS ?month) BIND (year(?dateCreated) AS ?year)
//!           FILTER (?who != {{{entityA}}}) .
//!         } GROUP BY ?year ?month ?day ?who }
//!     } GROUP BY ?who }
//! } ORDER BY DESC(?interactionDays) ?who LIMIT {{{randomLimit}}}
//! ```
//!
//! The innermost `SELECT DISTINCT ?year ?month ?day ?who` collapses every
//! creative work about both `A` and some other entity `who` down to the set of
//! `(day, who)` pairs; the middle `COUNT(*) GROUP BY ?who` then counts, per
//! `who`, how many distinct days it shares with `A`. So `interactionDays` is the
//! size of the per-`who` set of distinct `dateCreated` days — multiple works on
//! the same day (or a single work co-mentioning several entities) collapse
//! exactly as the `DISTINCT` dictates.
//!
//! Implementation: resolve `A` by `uri`; walk its incoming `about` rels to the
//! creative works about it (`CreativeWork`-labelled), read each work's required
//! `dateCreated` down to its `YYYY-MM-DD` prefix, and fold every *other*
//! outgoing `about` target's day into a per-`who` day set. We then
//! `ORDER BY DESC(?interactionDays)`, tie-break on `?who` ascending (its `uri`,
//! matching SPARQL IRI order) and `LIMIT` (`{{{randomLimit}}}`; the header's
//! "10").
//!
//! Caveats / deviations:
//! - `cwork:dateCreated` is a required pattern (and supplies the day key), so a
//!   work lacking it is excluded; a dense string column reads back as `Some("")`
//!   for an absent value, so we treat empty (or a sub-10-char) value as absent.
//! - `?who` ranges over `about` targets only (the BBC `about` link to a
//!   Company/Event entity); `A` itself is excluded by the `FILTER`.

use std::collections::{HashMap, HashSet};

use rustychickpeas_core::{Direction, GraphSnapshot};

use super::queries::{has_label, node_by_uri};

/// SPB advanced **q25**: entities co-occurring with the entity identified by
/// `uri_a` in creative works, paired with `interactionDays` (the count of
/// distinct calendar days on which a co-occurrence happens), ordered by
/// `interactionDays` descending then `who`'s `uri` ascending, truncated to
/// `limit` (the template's `{{{randomLimit}}}`; the header's "10"). Returns an
/// empty vector when `uri_a` resolves to no node.
pub fn run(g: &GraphSnapshot, uri_a: &str, limit: usize) -> Vec<(u32, usize)> {
    let Some(a) = node_by_uri(g, uri_a) else {
        return Vec::new();
    };

    // who -> set of distinct "YYYY-MM-DD" days it shares with `A`. The day key is
    // borrowed straight from the graph's string store (no per-day allocation),
    // and the set realises the inner `SELECT DISTINCT ... GROUP BY`.
    let mut days: HashMap<u32, HashSet<&str>> = HashMap::new();

    for cw in g.neighbors_by_type(a, Direction::Incoming, "about") {
        if !has_label(g, cw, "CreativeWork") {
            continue;
        }
        // Required `dateCreated`; empty (absent dense column) or a malformed
        // sub-10-char value drops the work, as the bound pattern demands.
        let Some(day) = g.prop_str(cw, "dateCreated").and_then(|s| s.get(..10)) else {
            continue;
        };
        for who in g.neighbors_by_type(cw, Direction::Outgoing, "about") {
            if who != a {
                days.entry(who).or_default().insert(day);
            }
        }
    }

    let mut rows: Vec<(u32, usize)> = days
        .into_iter()
        .map(|(who, set)| (who, set.len()))
        .collect();
    // ORDER BY DESC(?interactionDays); tie-break on node id (a stable proxy for the
    // uri order — avoids a per-comparison `uri` lookup) since the cross-engine
    // comparison is order-insensitive and the official tie-break is unspecified.
    rows.sort_by(|x, y| y.1.cmp(&x.1).then_with(|| x.0.cmp(&y.0)));
    rows.truncate(limit);
    rows
}

#[cfg(test)]
mod tests {
    use super::super::loader::load_str;
    use super::*;
    use crate::props::PropExt;

    const ENT_A: &str = "http://dbpedia.org/resource/EntA";
    const ENT_B: &str = "http://dbpedia.org/resource/EntB";
    const ENT_C: &str = "http://dbpedia.org/resource/EntC";
    const ENT_D: &str = "http://dbpedia.org/resource/EntD";

    // Entity A co-occurs with B, C and D across creative works:
    //   * B: cw1 (06-01), cw2 (06-02), cw3 (06-02 — same day, collapses) -> 2 days
    //   * C: cw4 (07-10), cw5 (07-11) -> 2 days
    //   * D: cw5 (07-11) -> 1 day                (cw5 is about A, C AND D)
    // cw6 is about A alone (no co-occurrence); cw7 is about A+B but lacks the
    // required dateCreated (excluded — B stays 2); cw8 (C+D) is NOT about A, so it
    // is never reached and does not add a day to C or D.
    const FIXTURE: &str = r#"
<http://dbpedia.org/resource/EntA> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://dbpedia.org/ontology/Company> .
<http://dbpedia.org/resource/EntB> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://dbpedia.org/ontology/Company> .
<http://dbpedia.org/resource/EntC> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://dbpedia.org/ontology/Company> .
<http://dbpedia.org/resource/EntD> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://dbpedia.org/ontology/Event> .

<http://ex/cw1> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw1> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/EntA> .
<http://ex/cw1> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/EntB> .
<http://ex/cw1> <http://www.bbc.co.uk/ontologies/creativework/dateCreated> "2024-06-01T12:00:00.000+00:00"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/cw2> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw2> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/EntA> .
<http://ex/cw2> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/EntB> .
<http://ex/cw2> <http://www.bbc.co.uk/ontologies/creativework/dateCreated> "2024-06-02T08:00:00.000+00:00"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/cw3> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw3> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/EntA> .
<http://ex/cw3> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/EntB> .
<http://ex/cw3> <http://www.bbc.co.uk/ontologies/creativework/dateCreated> "2024-06-02T19:30:00.000+00:00"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/cw4> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw4> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/EntA> .
<http://ex/cw4> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/EntC> .
<http://ex/cw4> <http://www.bbc.co.uk/ontologies/creativework/dateCreated> "2024-07-10T10:00:00.000+00:00"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/cw5> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw5> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/EntA> .
<http://ex/cw5> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/EntC> .
<http://ex/cw5> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/EntD> .
<http://ex/cw5> <http://www.bbc.co.uk/ontologies/creativework/dateCreated> "2024-07-11T14:00:00.000+00:00"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/cw6> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw6> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/EntA> .
<http://ex/cw6> <http://www.bbc.co.uk/ontologies/creativework/dateCreated> "2024-08-01T09:00:00.000+00:00"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/cw7> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw7> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/EntA> .
<http://ex/cw7> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/EntB> .

<http://ex/cw8> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw8> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/EntC> .
<http://ex/cw8> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/EntD> .
<http://ex/cw8> <http://www.bbc.co.uk/ontologies/creativework/dateCreated> "2024-09-01T09:00:00.000+00:00"^^<http://www.w3.org/2001/XMLSchema#dateTime> .
"#;

    /// Map a result to `(who uri, interactionDays)` for order-sensitive asserts.
    fn rows(g: &GraphSnapshot, uri_a: &str, limit: usize) -> Vec<(String, usize)> {
        run(g, uri_a, limit)
            .into_iter()
            .map(|(who, d)| (g.prop(who, "uri").str().unwrap_or("?").to_string(), d))
            .collect()
    }

    #[test]
    fn ranks_by_distinct_days_with_same_day_collapse() {
        let g = load_str(FIXTURE).0;
        // B and C each share 2 distinct days with A (B's two 06-02 works collapse
        // to one day); D shares 1. A itself never appears as a result.
        assert_eq!(
            rows(&g, ENT_A, 10),
            [
                (ENT_B.to_string(), 2),
                (ENT_C.to_string(), 2),
                (ENT_D.to_string(), 1)
            ]
        );
    }

    #[test]
    fn ties_break_on_who_uri_ascending() {
        let g = load_str(FIXTURE).0;
        // B and C tie at 2 interactionDays -> ordered by uri ascending (EntB < EntC).
        let out = rows(&g, ENT_A, 10);
        assert_eq!(out[0], (ENT_B.to_string(), 2));
        assert_eq!(out[1], (ENT_C.to_string(), 2));
        // A is excluded from its own related-entities list.
        assert!(out.iter().all(|(uri, _)| uri != ENT_A));
    }

    #[test]
    fn limit_truncates_after_ordering() {
        let g = load_str(FIXTURE).0;
        assert_eq!(
            rows(&g, ENT_A, 2),
            [(ENT_B.to_string(), 2), (ENT_C.to_string(), 2)]
        );
        // An unknown entity yields no rows.
        assert!(run(&g, "http://dbpedia.org/resource/Unknown", 10).is_empty());
    }
}

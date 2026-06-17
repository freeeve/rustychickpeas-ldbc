//! SPB advanced **q20** — full-text: "retrieve creative works (and their
//! properties) that contain a word in their `title` or `description`, newest
//! first, limited".
//!
//! Hand translation of `advanced/aggregation_standard/query20.txt` (no SPARQL
//! engine). The query body is:
//!
//! ```sparql
//! WHERE {
//!   ?cWork a cwork:CreativeWork ; a ?type ; cwork:title ?title ; cwork:description ?description .
//!   ?type rdfs:subClassOf cwork:CreativeWork .
//!   ?cWork cwork:dateModified ?dateModified .
//!   OPTIONAL { ... dateCreated / about / category / primaryContentOf ... }
//!   FILTER (CONTAINS(?title, {{word}}) || CONTAINS(?description, {{word}}))
//! } ORDER BY DESC(?dateModified) LIMIT {{randomLimit}}
//! ```
//!
//! Identity comes from the FILTER (word in title OR description), which we serve
//! from the core inverted index — the boolean union of `full_text_search(title)` and
//! `full_text_search(description)`, exactly as basic q8 does. `cwork:dateModified` is a required
//! pattern (and the sort key), so works lacking it are excluded; we then
//! `ORDER BY DESC(?dateModified)` (ISO-8601, hence lexicographic) and `LIMIT`.
//!
//! Caveats / deviations:
//! - CONTAINS vs token: SPB's `CONTAINS` is a substring test; our `full_text_search` matches
//!   whole-word tokens (case-insensitive), so a substring-of-a-word hit
//!   (e.g. "foot" in "football") would match in SPARQL but not here. Same caveat
//!   as q8.
//! - The required `cwork:title` / `cwork:description` patterns mean a work must
//!   carry BOTH fields; in the extract every CreativeWork does, and the full_text_search union
//!   already covers the FILTER, so we do not separately re-check both are present.
//! - The OPTIONAL projections (dateCreated / about / category / web document) only
//!   decorate rows in the CONSTRUCT; we return the ranked work ids.

use rustychickpeas_core::GraphSnapshot;

use crate::props::top_k_by_key;

/// Creative works whose `title` OR `description` contains `word` (core inverted
/// index, whole-word), ranked by `dateModified` descending (tie-broken by node id
/// ascending) and truncated to `limit` rows.
pub fn run(g: &GraphSnapshot, word: &str, limit: usize) -> Vec<u32> {
    let hits = &g.full_text_search("CreativeWork", "description", word) | &g.full_text_search("CreativeWork", "title", word);

    // `cwork:dateModified ?dateModified` is required and is the ORDER BY key, so a
    // work without it is excluded; carry the value to sort without re-lookup.
    // A node missing a dense string property reads back as `Some("")`, so treat
    // empty as absent for the required `dateModified` sort key.
    let rows: Vec<(u32, &str)> = hits
        .iter()
        .filter_map(|w| g.prop_str(w, "dateModified").map(|d| (w, d)))
        .collect();
    top_k_by_key(rows, limit)
        .into_iter()
        .map(|(w, _)| w)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::loader::load_str;
    use super::*;
    use crate::props::pstr;

    // Creative works matching "football" in title or description with assorted
    // `dateModified`, one "tennis"-only work, and one "football" work with no
    // `dateModified` (excluded by the required sort key).
    const FIXTURE: &str = r#"
<http://ex/cw1> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw1> <http://www.bbc.co.uk/ontologies/creativework/title> "London derby" .
<http://ex/cw1> <http://www.bbc.co.uk/ontologies/creativework/description> "a football match in london" .
<http://ex/cw1> <http://www.bbc.co.uk/ontologies/creativework/dateModified> "2024-06-01T12:00:00.000+00:00"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/cw2> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw2> <http://www.bbc.co.uk/ontologies/creativework/title> "Football weekly review" .
<http://ex/cw2> <http://www.bbc.co.uk/ontologies/creativework/description> "sports roundup" .
<http://ex/cw2> <http://www.bbc.co.uk/ontologies/creativework/dateModified> "2024-09-01T12:00:00.000+00:00"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/cw3> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw3> <http://www.bbc.co.uk/ontologies/creativework/title> "Football classics" .
<http://ex/cw3> <http://www.bbc.co.uk/ontologies/creativework/description> "archive games" .
<http://ex/cw3> <http://www.bbc.co.uk/ontologies/creativework/dateModified> "2024-01-01T12:00:00.000+00:00"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/cw4> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw4> <http://www.bbc.co.uk/ontologies/creativework/title> "Wimbledon final" .
<http://ex/cw4> <http://www.bbc.co.uk/ontologies/creativework/description> "tennis championship in london" .
<http://ex/cw4> <http://www.bbc.co.uk/ontologies/creativework/dateModified> "2025-01-01T12:00:00.000+00:00"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/cw5> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw5> <http://www.bbc.co.uk/ontologies/creativework/title> "Football extra" .
<http://ex/cw5> <http://www.bbc.co.uk/ontologies/creativework/description> "more coverage" .
"#;

    fn titles(g: &GraphSnapshot, works: &[u32]) -> Vec<String> {
        works
            .iter()
            .map(|&w| pstr(g, w, "title").unwrap_or("?").to_string())
            .collect()
    }

    #[test]
    fn full_text_ranked_by_date_modified_desc() {
        let g = load_str(FIXTURE).0;
        let works = run(&g, "football", 10);
        // newest `dateModified` first; the tennis-only work and the
        // `dateModified`-less "Football extra" are excluded.
        assert_eq!(
            titles(&g, &works),
            [
                "Football weekly review",
                "London derby",
                "Football classics"
            ]
        );
    }

    #[test]
    fn limit_truncates_after_ordering() {
        let g = load_str(FIXTURE).0;
        assert_eq!(
            titles(&g, &run(&g, "football", 2)),
            ["Football weekly review", "London derby"]
        );
    }

    #[test]
    fn matches_description_as_well_as_title() {
        let g = load_str(FIXTURE).0;
        // "tennis" appears only in cw4's description.
        assert_eq!(titles(&g, &run(&g, "tennis", 10)), ["Wimbledon final"]);
    }
}

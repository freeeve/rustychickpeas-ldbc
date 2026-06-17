//! SPB basic **q5** — "entities most tagged within a one-hour interval", with
//! restrictions on the creative-work type and audience.
//!
//! Official SPARQL (aggregation/query5.txt):
//!
//! ```text
//! SELECT DISTINCT (MAX(?topicPrefLabel) AS ?topicLabel) (COUNT(*) AS ?cnt)
//! WHERE {
//!   ?creativeWork a {{{cwType}}} ;
//!      cwork:tag ?topic ;
//!      cwork:dateModified ?dt ;
//!      cwork:audience {{{cwAudience}}} .
//!   FILTER (?dt > {{{cwStartDateTime}}} && ?dt < {{{cwEndDateTime}}}) .
//!   ?topic ldbcspb:prefLabel ?topicPrefLabel .
//! }
//! GROUP BY ?topic ORDER BY DESC(?cnt)
//! ```
//!
//! Plain English: among creative works of the given type, addressed to the given
//! audience and modified inside the `(start, end)` window, count how many tag
//! each topic entity; return each topic's label and that count, most-tagged
//! first.
//!
//! Vocabulary mapping (no SPARQL / RDFS engine):
//!   * `cwork:tag` is the super-property of `cwork:about` / `cwork:mentions`;
//!     lacking sub-property reasoning we evaluate it as the union of the `about`,
//!     `mentions` and direct `tag` out-edges, deduped per work — so a work that
//!     tags one topic via both `about` and `mentions` counts once, matching RDF
//!     triple-set semantics for the entailed `cwork:tag` triple.
//!   * `?topic ldbcspb:prefLabel ?topicPrefLabel` is an inner join: only topics
//!     carrying a display label participate. The extract materializes that label
//!     from `rdfs:label` onto the `Company` / `Event` topic nodes, read here as
//!     the `label` property (`MAX` collapses to that single value).
//!   * `a {{{cwType}}}` and `cwork:audience {{{cwAudience}}}` are equality
//!     restrictions: the work's type label, and an `audience` out-edge to the
//!     node with the requested uri.
//!   * the FILTER is a `dateModified` range, compared as epoch-ms via `parse_ms`.

use std::collections::{HashMap, HashSet};

use rustychickpeas_core::{Direction, GraphSnapshot};

use crate::props::{parse_ms, pstr};

/// The sub-properties making up `cwork:tag` (`tag` itself is the materialized
/// union of these, so traversing it as well would be redundant work).
const TAG_PREDICATES: [&str; 2] = ["about", "mentions"];

/// Run SPB basic q5: per topic, the number of creative works tagging it, subject
/// to the type / audience / dateModified restrictions, ordered by count
/// descending (ties broken by label for a stable result).
///
/// * `cw_type` — the work's type as a label local-name (e.g. `"BlogPost"`);
///   `None` drops the `a {{{cwType}}}` restriction.
/// * `audience_uri` — the audience the work is `cwork:audience`-linked to;
///   `None` drops that restriction.
/// * `start` / `end` — the exclusive `dateModified` window (ISO-8601 strings).
pub fn run(
    g: &GraphSnapshot,
    cw_type: Option<&str>,
    audience_uri: Option<&str>,
    start: &str,
    end: &str,
) -> Vec<(String, usize)> {
    let start_ms = parse_ms(start);
    let end_ms = parse_ms(end);

    // `?creativeWork a {{{cwType}}}` — iterate that type's nodes directly (or all
    // CreativeWorks when unrestricted) rather than scanning every work and testing
    // the label.
    let Some(works) = g.nodes_with_label(cw_type.unwrap_or("CreativeWork")) else {
        return Vec::new();
    };

    // topic node -> number of distinct creative works tagging it.
    let mut counts: HashMap<u32, usize> = HashMap::new();

    for cw in works.iter() {
        // cwork:audience {{{cwAudience}}}
        if let Some(aud) = audience_uri {
            let matches = g
                .neighbors_by_type(cw, Direction::Outgoing, "audience")
                .any(|a| pstr(g, a, "uri") == Some(aud));
            if !matches {
                continue;
            }
        }
        // cwork:dateModified ?dt . FILTER (?dt > start && ?dt < end)
        let Some(dt) = pstr(g, cw, "dateModified") else {
            continue;
        };
        let dt_ms = parse_ms(dt);
        if !(dt_ms > start_ms && dt_ms < end_ms) {
            continue;
        }
        // cwork:tag ?topic . ?topic ldbcspb:prefLabel ?topicPrefLabel
        let mut topics: HashSet<u32> = HashSet::new();
        for pred in TAG_PREDICATES {
            topics.extend(g.neighbors_by_type(cw, Direction::Outgoing, pred));
        }
        for topic in topics {
            if pstr(g, topic, "label").is_some() {
                *counts.entry(topic).or_insert(0) += 1;
            }
        }
    }

    let mut rows: Vec<(String, usize)> = counts
        .into_iter()
        .map(|(topic, cnt)| (pstr(g, topic, "label").unwrap_or("?").to_string(), cnt))
        .collect();
    // ORDER BY DESC(?cnt), then label ascending for determinism.
    rows.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    rows
}

#[cfg(test)]
mod tests {
    use super::super::loader::load_str;
    use super::*;

    // Two Company topics with labels (Acme, Globex) and one without (Hidden,
    // excluded by the prefLabel join). Works of type BlogPost / NewsItem tag them
    // via about/mentions, with a National/International audience and a
    // dateModified timestamp.
    const FIXTURE: &str = r#"
<http://dbpedia.org/resource/Acme> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://dbpedia.org/ontology/Company> .
<http://dbpedia.org/resource/Acme> <http://www.w3.org/2000/01/rdf-schema#label> "Acme Corp" .
<http://dbpedia.org/resource/Globex> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://dbpedia.org/ontology/Company> .
<http://dbpedia.org/resource/Globex> <http://www.w3.org/2000/01/rdf-schema#label> "Globex" .
<http://dbpedia.org/resource/Hidden> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://dbpedia.org/ontology/Company> .

<http://ex/cw1> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw1> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/BlogPost> .
<http://ex/cw1> <http://www.bbc.co.uk/ontologies/creativework/dateModified> "2012-01-01T10:00:00.000Z" .
<http://ex/cw1> <http://www.bbc.co.uk/ontologies/creativework/audience> <http://www.bbc.co.uk/ontologies/creativework/NationalAudience> .
<http://ex/cw1> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/Acme> .
<http://ex/cw1> <http://www.bbc.co.uk/ontologies/creativework/mentions> <http://dbpedia.org/resource/Globex> .

<http://ex/cw2> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw2> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/BlogPost> .
<http://ex/cw2> <http://www.bbc.co.uk/ontologies/creativework/dateModified> "2012-01-01T10:30:00.000Z" .
<http://ex/cw2> <http://www.bbc.co.uk/ontologies/creativework/audience> <http://www.bbc.co.uk/ontologies/creativework/NationalAudience> .
<http://ex/cw2> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/Acme> .

<http://ex/cw7> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw7> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/BlogPost> .
<http://ex/cw7> <http://www.bbc.co.uk/ontologies/creativework/dateModified> "2012-01-01T10:50:00.000Z" .
<http://ex/cw7> <http://www.bbc.co.uk/ontologies/creativework/audience> <http://www.bbc.co.uk/ontologies/creativework/NationalAudience> .
<http://ex/cw7> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/Acme> .

<http://ex/cw3> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw3> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/BlogPost> .
<http://ex/cw3> <http://www.bbc.co.uk/ontologies/creativework/dateModified> "2012-01-01T10:45:00.000Z" .
<http://ex/cw3> <http://www.bbc.co.uk/ontologies/creativework/audience> <http://www.bbc.co.uk/ontologies/creativework/NationalAudience> .
<http://ex/cw3> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/Globex> .
<http://ex/cw3> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/Hidden> .

<http://ex/cw4> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw4> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/BlogPost> .
<http://ex/cw4> <http://www.bbc.co.uk/ontologies/creativework/dateModified> "2013-06-01T10:00:00.000Z" .
<http://ex/cw4> <http://www.bbc.co.uk/ontologies/creativework/audience> <http://www.bbc.co.uk/ontologies/creativework/NationalAudience> .
<http://ex/cw4> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/Acme> .

<http://ex/cw5> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw5> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/BlogPost> .
<http://ex/cw5> <http://www.bbc.co.uk/ontologies/creativework/dateModified> "2012-01-01T10:05:00.000Z" .
<http://ex/cw5> <http://www.bbc.co.uk/ontologies/creativework/audience> <http://www.bbc.co.uk/ontologies/creativework/InternationalAudience> .
<http://ex/cw5> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/Acme> .

<http://ex/cw6> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw6> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/NewsItem> .
<http://ex/cw6> <http://www.bbc.co.uk/ontologies/creativework/dateModified> "2012-01-01T10:15:00.000Z" .
<http://ex/cw6> <http://www.bbc.co.uk/ontologies/creativework/audience> <http://www.bbc.co.uk/ontologies/creativework/NationalAudience> .
<http://ex/cw6> <http://www.bbc.co.uk/ontologies/creativework/mentions> <http://dbpedia.org/resource/Globex> .
"#;

    const NAT: &str = "http://www.bbc.co.uk/ontologies/creativework/NationalAudience";
    const WIN_START: &str = "2012-01-01T09:00:00.000Z";
    const WIN_END: &str = "2012-01-01T11:00:00.000Z";

    #[test]
    fn counts_tagging_works_per_topic_with_all_restrictions() {
        let g = load_str(FIXTURE).0;
        // BlogPost + National + window: cw1, cw2, cw7, cw3 qualify.
        //   Acme   <- cw1(about), cw2(about), cw7(about)        = 3
        //   Globex <- cw1(mentions), cw3(about)                 = 2
        //   Hidden has no label -> excluded by the prefLabel join.
        // cw4 (out of window), cw5 (International), cw6 (NewsItem) are excluded.
        let rows = run(&g, Some("BlogPost"), Some(NAT), WIN_START, WIN_END);
        assert_eq!(rows, vec![("Acme Corp".to_string(), 3), ("Globex".to_string(), 2)]);
    }

    #[test]
    fn dropping_the_type_restriction_folds_in_other_work_types() {
        let g = load_str(FIXTURE).0;
        // No type restriction adds cw6 (NewsItem, National, in window) -> Globex 3.
        let rows = run(&g, None, Some(NAT), WIN_START, WIN_END);
        assert_eq!(rows, vec![("Acme Corp".to_string(), 3), ("Globex".to_string(), 3)]);
    }

    #[test]
    fn dropping_the_audience_restriction_folds_in_other_audiences() {
        let g = load_str(FIXTURE).0;
        // No audience restriction adds cw5 (International, about Acme) -> Acme 4.
        let rows = run(&g, Some("BlogPost"), None, WIN_START, WIN_END);
        assert_eq!(rows, vec![("Acme Corp".to_string(), 4), ("Globex".to_string(), 2)]);
    }

    #[test]
    fn the_window_filter_excludes_works_outside_it() {
        let g = load_str(FIXTURE).0;
        // Narrow to (10:20, 10:48): only cw2 (10:30 -> Acme) and cw3 (10:45 -> Globex).
        let rows = run(
            &g,
            Some("BlogPost"),
            Some(NAT),
            "2012-01-01T10:20:00.000Z",
            "2012-01-01T10:48:00.000Z",
        );
        assert_eq!(rows, vec![("Acme Corp".to_string(), 1), ("Globex".to_string(), 1)]);
    }
}

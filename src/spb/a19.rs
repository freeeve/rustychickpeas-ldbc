//! SPB advanced **q19** — "most popular topics tagged in a date-time range,
//! ordered by most-recent modification", restricted by creative-work type and
//! audience.
//!
//! Official SPARQL (advanced/aggregation_standard/query19.txt):
//!
//! ```text
//! SELECT ?topic (COUNT(*) AS ?topicsCount) (MAX(?dateModif) AS ?maxDate) {
//!   ?cwork a {{{cwType}}} .
//!   ?cwork cwork:audience {{{cwAudience}}} .
//!   ?cwork cwork:tag ?topic .
//!   ?cwork cwork:dateModified ?dateModif .
//!   {{{cwFilterDateModifiedCondition}}}
//! }
//! GROUP BY ?topic
//! ORDER BY DESC(?maxDate) DESC(?topicsCount)
//! LIMIT {{{randomLimit}}}
//! ```
//!
//! Plain English: among creative works of the given type, addressed to the given
//! audience and modified inside the date range, count how many tag each topic and
//! track the most-recent modification of those works; return the topics ordered
//! by that newest date (then by count), capped at `limit`.
//!
//! This is q5's faceted count over the same `cwork:tag` expansion, with two
//! differences that matter:
//!   * it projects/sorts on `MAX(?dateModif)` (newest tagging work) first, count
//!     second — so a less-tagged-but-fresher topic outranks a stale popular one;
//!   * **it has no `ldbcspb:prefLabel` join** — unlike q5, topics are *not*
//!     required to carry a display label, so label-less tag targets stay in the
//!     result. We render `?topic` as its `label` when present, else its `uri`
//!     (the value `?topic` actually binds to), and never drop a topic.
//!
//! Vocabulary mapping (no SPARQL / RDFS engine): `cwork:tag` is evaluated as the
//! union of the `about` / `mentions` / direct `tag` out-rels, deduped per work
//! (RDF triple-set semantics); `a {{{cwType}}}` / `cwork:audience` are equality
//! restrictions on the work's type label and `audience` out-rel target; the date
//! condition is an inclusive `[start, end]` window on `dateModified`, compared as
//! epoch-ms via `parse_ms`.

use std::collections::{HashMap, HashSet};

use rustychickpeas_core::{Direction, GraphSnapshot};

use super::queries::has_label;
use crate::props::{parse_ms, PropExt};

/// Rels making up `cwork:tag` once its `about` / `mentions` sub-properties are
/// folded in (no RDFS engine), plus any direct `tag` rel.
const TAG_PREDICATES: [&str; 3] = ["about", "mentions", "tag"];

/// Run SPB advanced q19: the top `limit` topics by most-recent tagging-work
/// modification date (then by tag count), subject to the type / audience /
/// dateModified restrictions. Each row is `(topic, count, maxDateModified)` where
/// `topic` is the topic's `label` if it has one, otherwise its `uri`.
///
/// * `cw_type` — the work's type as a label local-name (e.g. `"BlogPost"`);
///   `None` drops the `a {{{cwType}}}` restriction.
/// * `audience_uri` — the audience the work is `cwork:audience`-linked to;
///   `None` drops that restriction.
/// * `start` / `end` — the inclusive `dateModified` window (ISO-8601 strings).
/// * `limit` — the `LIMIT`; `0` returns no rows.
pub fn run(
    g: &GraphSnapshot,
    cw_type: Option<&str>,
    audience_uri: Option<&str>,
    start: &str,
    end: &str,
    limit: usize,
) -> Vec<(String, usize, String)> {
    let start_ms = parse_ms(start);
    let end_ms = parse_ms(end);

    let Some(works) = g.nodes_with_label("CreativeWork") else {
        return Vec::new();
    };

    // topic node -> (tag count, newest dateModified as epoch-ms, that date string).
    let mut acc: HashMap<u32, (usize, i64, &str)> = HashMap::new();

    for cw in works.iter() {
        // ?cwork a {{{cwType}}}
        if cw_type.is_some_and(|ty| !has_label(g, cw, ty)) {
            continue;
        }
        // ?cwork cwork:audience {{{cwAudience}}}
        if let Some(aud) = audience_uri {
            let matches = g
                .neighbors_by_type(cw, Direction::Outgoing, "audience")
                .any(|a| g.prop(a, "uri").str() == Some(aud));
            if !matches {
                continue;
            }
        }
        // ?cwork cwork:dateModified ?dateModif . FILTER (start <= ?dateModif <= end)
        let Some(dt) = g.prop(cw, "dateModified").str() else {
            continue;
        };
        let dt_ms = parse_ms(dt);
        if dt_ms < start_ms || dt_ms > end_ms {
            continue;
        }
        // ?cwork cwork:tag ?topic
        let mut topics: HashSet<u32> = HashSet::new();
        for pred in TAG_PREDICATES {
            topics.extend(g.neighbors_by_type(cw, Direction::Outgoing, pred));
        }
        for topic in topics {
            let e = acc.entry(topic).or_insert((0, i64::MIN, dt));
            e.0 += 1;
            if dt_ms > e.1 {
                e.1 = dt_ms;
                e.2 = dt;
            }
        }
    }

    // Sort / truncate on node ids (ms desc, count desc, node asc) and render the
    // display name + date strings only for the kept rows.
    let mut rows: Vec<(u32, usize, i64, &str)> = acc
        .into_iter()
        .map(|(topic, (cnt, ms, date))| (topic, cnt, ms, date))
        .collect();
    rows.sort_by(|a, b| {
        b.2.cmp(&a.2)
            .then_with(|| b.1.cmp(&a.1))
            .then_with(|| a.0.cmp(&b.0))
    });
    rows.truncate(limit);
    rows.into_iter()
        .map(|(topic, cnt, _ms, date)| {
            let name = g.prop(topic, "label").str()
                .or_else(|| g.prop(topic, "uri").str())
                .unwrap_or("?")
                .to_string();
            (name, cnt, date.to_string())
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::loader::load_str;
    use super::*;

    // Acme (label) tagged by 3 works peaking at 10:20; Globex (label) tagged by 2
    // peaking at 10:40; Hidden (NO label) tagged by 1 at 10:05. Two decoys: an
    // International-audience work and an out-of-window work that must be excluded.
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

<http://ex/cw2> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw2> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/BlogPost> .
<http://ex/cw2> <http://www.bbc.co.uk/ontologies/creativework/dateModified> "2012-01-01T10:10:00.000Z" .
<http://ex/cw2> <http://www.bbc.co.uk/ontologies/creativework/audience> <http://www.bbc.co.uk/ontologies/creativework/NationalAudience> .
<http://ex/cw2> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/Acme> .

<http://ex/cw3> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw3> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/BlogPost> .
<http://ex/cw3> <http://www.bbc.co.uk/ontologies/creativework/dateModified> "2012-01-01T10:20:00.000Z" .
<http://ex/cw3> <http://www.bbc.co.uk/ontologies/creativework/audience> <http://www.bbc.co.uk/ontologies/creativework/NationalAudience> .
<http://ex/cw3> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/Acme> .

<http://ex/cw4> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw4> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/BlogPost> .
<http://ex/cw4> <http://www.bbc.co.uk/ontologies/creativework/dateModified> "2012-01-01T10:30:00.000Z" .
<http://ex/cw4> <http://www.bbc.co.uk/ontologies/creativework/audience> <http://www.bbc.co.uk/ontologies/creativework/NationalAudience> .
<http://ex/cw4> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/Globex> .

<http://ex/cw5> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw5> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/BlogPost> .
<http://ex/cw5> <http://www.bbc.co.uk/ontologies/creativework/dateModified> "2012-01-01T10:40:00.000Z" .
<http://ex/cw5> <http://www.bbc.co.uk/ontologies/creativework/audience> <http://www.bbc.co.uk/ontologies/creativework/NationalAudience> .
<http://ex/cw5> <http://www.bbc.co.uk/ontologies/creativework/mentions> <http://dbpedia.org/resource/Globex> .

<http://ex/cw6> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw6> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/BlogPost> .
<http://ex/cw6> <http://www.bbc.co.uk/ontologies/creativework/dateModified> "2012-01-01T10:05:00.000Z" .
<http://ex/cw6> <http://www.bbc.co.uk/ontologies/creativework/audience> <http://www.bbc.co.uk/ontologies/creativework/NationalAudience> .
<http://ex/cw6> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/Hidden> .

<http://ex/cw7> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw7> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/BlogPost> .
<http://ex/cw7> <http://www.bbc.co.uk/ontologies/creativework/dateModified> "2012-01-01T10:50:00.000Z" .
<http://ex/cw7> <http://www.bbc.co.uk/ontologies/creativework/audience> <http://www.bbc.co.uk/ontologies/creativework/InternationalAudience> .
<http://ex/cw7> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/Acme> .

<http://ex/cw8> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw8> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/BlogPost> .
<http://ex/cw8> <http://www.bbc.co.uk/ontologies/creativework/dateModified> "2013-01-01T10:00:00.000Z" .
<http://ex/cw8> <http://www.bbc.co.uk/ontologies/creativework/audience> <http://www.bbc.co.uk/ontologies/creativework/NationalAudience> .
<http://ex/cw8> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/Globex> .
"#;

    const NAT: &str = "http://www.bbc.co.uk/ontologies/creativework/NationalAudience";
    const WIN_START: &str = "2012-01-01T09:00:00.000Z";
    const WIN_END: &str = "2012-01-01T11:00:00.000Z";

    #[test]
    fn orders_by_newest_modification_then_count_keeping_labelless_topics() {
        let g = load_str(FIXTURE).0;
        let rows = run(&g, Some("BlogPost"), Some(NAT), WIN_START, WIN_END, 10);
        // Globex (max 10:40, 2 works) outranks the *more-tagged* Acme (max 10:20,
        // 3 works) because MAX(dateModified) sorts first. Hidden has no label, so
        // it is rendered by uri and — unlike q5 — is NOT dropped.
        // cw7 (International) and cw8 (2013, out of window) are excluded, so Acme
        // stays at 3 / 10:20 and Globex at 2 / 10:40.
        assert_eq!(
            rows,
            vec![
                (
                    "Globex".to_string(),
                    2,
                    "2012-01-01T10:40:00.000Z".to_string()
                ),
                (
                    "Acme Corp".to_string(),
                    3,
                    "2012-01-01T10:20:00.000Z".to_string()
                ),
                (
                    "http://dbpedia.org/resource/Hidden".to_string(),
                    1,
                    "2012-01-01T10:05:00.000Z".to_string()
                ),
            ]
        );
    }

    #[test]
    fn limit_truncates_after_ordering() {
        let g = load_str(FIXTURE).0;
        let rows = run(&g, Some("BlogPost"), Some(NAT), WIN_START, WIN_END, 2);
        assert_eq!(
            rows,
            vec![
                (
                    "Globex".to_string(),
                    2,
                    "2012-01-01T10:40:00.000Z".to_string()
                ),
                (
                    "Acme Corp".to_string(),
                    3,
                    "2012-01-01T10:20:00.000Z".to_string()
                ),
            ]
        );
    }

    #[test]
    fn narrowing_the_window_excludes_works_and_drops_topics() {
        let g = load_str(FIXTURE).0;
        // [10:25, 10:45] keeps only cw4 (10:30) and cw5 (10:40), both Globex;
        // Acme (<=10:20) and Hidden (10:05) drop out.
        let rows = run(
            &g,
            Some("BlogPost"),
            Some(NAT),
            "2012-01-01T10:25:00.000Z",
            "2012-01-01T10:45:00.000Z",
            10,
        );
        assert_eq!(
            rows,
            vec![(
                "Globex".to_string(),
                2,
                "2012-01-01T10:40:00.000Z".to_string()
            )]
        );
    }
}

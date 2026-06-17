//! SPB advanced **q10** — the creative works with the most `mentions`.
//!
//! Hand translation of `advanced/aggregation_standard/query10.txt`:
//! ```sparql
//! SELECT ?creativeWork ?mentionsCount ?dateCreated ?thing WHERE {
//!   { SELECT (MAX(?mc) AS ?maxMentions) WHERE {
//!       SELECT ?cw (COUNT(?m) AS ?mc) WHERE { ?cw cwork:mentions ?m . }
//!       GROUP BY ?cw } }
//!   ?creativeWork cwork:mentions ?thing ; cwork:dateCreated ?dateCreated .
//!   { SELECT ?creativeWork (COUNT(?m) AS ?mentionsCount) WHERE {
//!       ?creativeWork cwork:mentions ?m . } GROUP BY ?creativeWork }
//!   FILTER (?mentionsCount = ?maxMentions) .
//! }
//! ```
//!
//! We fold each creative work's outgoing `mentions` degree, take the maximum
//! across all works, and return every work that both attains it and carries a
//! `dateCreated` (the query's required triple pattern; a work without one is
//! filtered even if it ties the maximum). The `?dateCreated` / `?thing`
//! decorations are not part of the identity, so we return `(work_uri,
//! mentions_count)` ordered by uri.

use rustychickpeas_core::{Direction, GraphSnapshot};

use crate::props::pstr;

/// Creative works whose outgoing `mentions` count equals the maximum over all
/// works and which carry a `dateCreated`. Returned as `(work_uri,
/// mentions_count)` ordered by uri ascending, truncated to `limit`.
pub fn run(g: &GraphSnapshot, limit: usize) -> Vec<(String, usize)> {
    let Some(works) = g.nodes_with_label("CreativeWork") else {
        return Vec::new();
    };
    let counts: Vec<(u32, usize)> = works
        .iter()
        .map(|w| {
            (
                w,
                g.neighbors_by_type(w, Direction::Outgoing, "mentions")
                    .count(),
            )
        })
        .collect();
    let max = counts.iter().map(|&(_, n)| n).max().unwrap_or(0);
    if max == 0 {
        return Vec::new();
    }
    let mut rows: Vec<(String, usize)> = counts
        .into_iter()
        .filter(|&(w, n)| n == max && g.prop_str(w, "dateCreated").is_some())
        .map(|(w, n)| (pstr(g, w, "uri").unwrap_or("?").to_string(), n))
        .collect();
    rows.sort_by(|a, b| a.0.cmp(&b.0));
    rows.truncate(limit);
    rows
}

#[cfg(test)]
mod tests {
    use super::super::loader::load_str;
    use super::*;

    // Four works: cw1 mentions 1 thing, cw2/cw3/cw4 mention 3. cw1..cw3 carry a
    // dateCreated; cw4 attains the maximum but lacks one, so it is filtered.
    const FIXTURE: &str = r#"
<http://ex/cw1> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/CreativeWork> .
<http://ex/cw1> <http://bbc/mentions> <http://ex/t1> .
<http://ex/cw1> <http://bbc/dateCreated> "2011-01-01T00:00:00.000Z"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/cw2> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/CreativeWork> .
<http://ex/cw2> <http://bbc/mentions> <http://ex/t1> .
<http://ex/cw2> <http://bbc/mentions> <http://ex/t2> .
<http://ex/cw2> <http://bbc/mentions> <http://ex/t3> .
<http://ex/cw2> <http://bbc/dateCreated> "2011-02-01T00:00:00.000Z"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/cw3> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/CreativeWork> .
<http://ex/cw3> <http://bbc/mentions> <http://ex/t1> .
<http://ex/cw3> <http://bbc/mentions> <http://ex/t2> .
<http://ex/cw3> <http://bbc/mentions> <http://ex/t3> .
<http://ex/cw3> <http://bbc/dateCreated> "2011-03-01T00:00:00.000Z"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/cw4> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/CreativeWork> .
<http://ex/cw4> <http://bbc/mentions> <http://ex/t1> .
<http://ex/cw4> <http://bbc/mentions> <http://ex/t2> .
<http://ex/cw4> <http://bbc/mentions> <http://ex/t3> .
"#;

    #[test]
    fn returns_max_mention_works_with_date_created() {
        let g = load_str(FIXTURE).0;
        let rows = run(&g, 10);
        // max mentions = 3 (cw2, cw3, cw4); cw1 has only 1, cw4 lacks dateCreated.
        assert_eq!(
            rows,
            vec![
                ("http://ex/cw2".to_string(), 3),
                ("http://ex/cw3".to_string(), 3)
            ]
        );
    }
}

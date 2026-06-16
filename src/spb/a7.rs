//! SPB advanced **q7** — the most mentioned topics among creative works that are
//! the primary content of more than a threshold number of resources.
//!
//! Hand translation of `advanced/aggregation_standard/query7.txt`:
//! ```sparql
//! SELECT ?mentions (COUNT(*) AS ?count) WHERE {
//!   ?creativeWork cwork:mentions ?mentions .
//!   { SELECT ?creativeWork (COUNT(*) AS ?pcCount) WHERE {
//!       ?creativeWork bbc:primaryContentOf ?pc .
//!     } GROUP BY ?creativeWork }
//!   FILTER (?pcCount > {{{cwPrimaryContentThreshold}}}) .
//! } GROUP BY ?mentions ORDER BY DESC(?count) LIMIT 10
//! ```
//!
//! No RDFS materialization here — both `primaryContentOf` and `mentions` are read
//! as plain outgoing edges. The inner sub-select degenerates to an out-degree:
//! keep works whose outgoing `primaryContentOf` count is strictly greater than
//! `min_primary_content`, then tally their outgoing `mentions` targets, counting
//! each `(work, mentions)` pair (the outer `COUNT(*)`).

use std::collections::HashMap;

use rustychickpeas_core::{Direction, GraphSnapshot};

use crate::props::pstr;

/// Mention targets ranked by how many qualifying works mention them, where a work
/// qualifies when its outgoing `primaryContentOf` edge count is strictly greater
/// than `min_primary_content`. Returned as `(mentions_uri, count)` ordered by count
/// descending then uri, truncated to `limit` (the template's `LIMIT 10`).
pub fn run(g: &GraphSnapshot, min_primary_content: usize, limit: usize) -> Vec<(String, usize)> {
    let Some(works) = g.nodes_with_label("CreativeWork") else {
        return Vec::new();
    };
    let mut counts: HashMap<u32, usize> = HashMap::new();
    for w in works.iter() {
        let pc_count = g.neighbors_by_type(w, Direction::Outgoing, "primaryContentOf").count();
        if pc_count <= min_primary_content {
            continue;
        }
        for m in g.neighbors_by_type(w, Direction::Outgoing, "mentions") {
            *counts.entry(m).or_default() += 1;
        }
    }
    let mut rows: Vec<(String, usize)> = counts
        .into_iter()
        .map(|(m, n)| (pstr(g, m, "uri").unwrap_or("?").to_string(), n))
        .collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    rows.truncate(limit);
    rows
}

#[cfg(test)]
mod tests {
    use super::super::loader::load_str;
    use super::*;

    // Three works with primaryContentOf out-degrees 2, 3, 1 mentioning shared
    // Features; with threshold 1 only the first two qualify.
    const FIXTURE: &str = r#"
<http://ex/cw1> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/CreativeWork> .
<http://ex/cw1> <http://bbc/primaryContentOf> <http://ex/pc1> .
<http://ex/cw1> <http://bbc/primaryContentOf> <http://ex/pc2> .
<http://ex/cw1> <http://bbc/mentions> <http://ex/London> .
<http://ex/cw1> <http://bbc/mentions> <http://ex/Paris> .

<http://ex/cw2> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/CreativeWork> .
<http://ex/cw2> <http://bbc/primaryContentOf> <http://ex/pc3> .
<http://ex/cw2> <http://bbc/primaryContentOf> <http://ex/pc4> .
<http://ex/cw2> <http://bbc/primaryContentOf> <http://ex/pc5> .
<http://ex/cw2> <http://bbc/mentions> <http://ex/London> .

<http://ex/cw3> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/CreativeWork> .
<http://ex/cw3> <http://bbc/primaryContentOf> <http://ex/pc6> .
<http://ex/cw3> <http://bbc/mentions> <http://ex/London> .
<http://ex/cw3> <http://bbc/mentions> <http://ex/Berlin> .

<http://ex/London> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://geonames/Feature> .
<http://ex/Paris> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://geonames/Feature> .
<http://ex/Berlin> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://geonames/Feature> .
"#;

    #[test]
    fn counts_mentions_over_primary_content_threshold() {
        let g = load_str(FIXTURE).0;
        // threshold 1 -> only cw1 (out-degree 2) and cw2 (out-degree 3) qualify;
        // cw3 (out-degree 1) is excluded. London is mentioned by both, Paris by cw1.
        let rows = run(&g, 1, 10);
        assert_eq!(
            rows,
            vec![("http://ex/London".to_string(), 2), ("http://ex/Paris".to_string(), 1)]
        );
    }
}

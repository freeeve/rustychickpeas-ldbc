//! SPB advanced **q13** — creative works and their tags, restricted to two
//! categories.
//!
//! Hand translation of `advanced/aggregation_standard/query13.txt`:
//! ```sparql
//! SELECT DISTINCT ?thing ?tag ?category ?dateModified WHERE {
//!   ?thing a cwork:CreativeWork ; cwork:tag ?tag ;
//!     cwork:category ?category ; cwork:dateModified ?dateModified .
//!   FILTER (?category = {{{cat1}}} || ?category = {{{cat2}}})
//! } LIMIT 100
//! ```
//!
//! `cwork:tag` is the RDFS super-property of `cwork:about` / `cwork:mentions`,
//! materialized by the loader, so each `about`/`mentions` edge is also a `tag`
//! edge we read directly. `?category` and `?dateModified` are bound (the work
//! must carry a category edge to one of the two pinned uris and a non-empty
//! `dateModified`) but the `SELECT DISTINCT` identity is `(?thing, ?tag)` — the
//! pair we return, one row per distinct tag target.

use rustychickpeas_core::{Direction, GraphSnapshot};

use crate::props::pstr;

/// `(work_uri, tag_uri)` pairs for `CreativeWork`s `category`-linked to `cat1` or
/// `cat2` with a non-empty `dateModified`, one per distinct `tag` target (the
/// materialized super-property of about/mentions). Sorted by `(work, tag)` and
/// truncated to `limit` (the template's `LIMIT 100`).
pub fn run(g: &GraphSnapshot, cat1: &str, cat2: &str, limit: usize) -> Vec<(String, String)> {
    let Some(works) = g.nodes_with_label("CreativeWork") else {
        return Vec::new();
    };
    let mut rows: Vec<(String, String)> = Vec::new();
    for w in works.iter() {
        let in_category = g.neighbors_by_type(w, Direction::Outgoing, "category").any(|c| {
            let u = pstr(g, c, "uri");
            u == Some(cat1) || u == Some(cat2)
        });
        if !in_category {
            continue;
        }
        if pstr(g, w, "dateModified").filter(|s| !s.is_empty()).is_none() {
            continue;
        }
        let Some(work_uri) = pstr(g, w, "uri").map(str::to_string) else {
            continue;
        };
        // cwork:tag — the materialized super-property of about/mentions.
        for tag in g.neighbors_by_type(w, Direction::Outgoing, "tag") {
            if let Some(tag_uri) = pstr(g, tag, "uri") {
                rows.push((work_uri.clone(), tag_uri.to_string()));
            }
        }
    }
    rows.sort();
    rows.dedup(); // SELECT DISTINCT over (?thing, ?tag)
    rows.truncate(limit);
    rows
}

#[cfg(test)]
mod tests {
    use super::super::loader::load_str;
    use super::*;

    // TBox (about/mentions subPropertyOf tag) + one work in cat Sports tagging two
    // entities (about + mentions) and one work in the excluded cat Politics.
    const FIXTURE: &str = r#"
<http://bbc/about> <http://www.w3.org/2000/01/rdf-schema#subPropertyOf> <http://bbc/tag> .
<http://bbc/mentions> <http://www.w3.org/2000/01/rdf-schema#subPropertyOf> <http://bbc/tag> .

<http://ex/cw1> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/CreativeWork> .
<http://ex/cw1> <http://bbc/category> <http://cat/Sports> .
<http://ex/cw1> <http://bbc/about> <http://ex/Acme> .
<http://ex/cw1> <http://bbc/mentions> <http://ex/London> .
<http://ex/cw1> <http://bbc/dateModified> "2011-04-01T00:00:00.000Z"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/cw2> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/CreativeWork> .
<http://ex/cw2> <http://bbc/category> <http://cat/Politics> .
<http://ex/cw2> <http://bbc/about> <http://ex/Globex> .
<http://ex/cw2> <http://bbc/dateModified> "2011-04-02T00:00:00.000Z"^^<http://www.w3.org/2001/XMLSchema#dateTime> .
"#;

    #[test]
    fn one_row_per_tag_for_works_in_either_category() {
        let g = load_str(FIXTURE).0;
        // cw1 (Sports) tags Acme (about) + London (mentions) -> two rows; cw2
        // (Politics) is excluded.
        let rows = run(&g, "http://cat/Sports", "http://cat/Weather", 100);
        assert_eq!(
            rows,
            vec![
                ("http://ex/cw1".to_string(), "http://ex/Acme".to_string()),
                ("http://ex/cw1".to_string(), "http://ex/London".to_string()),
            ]
        );
    }

    #[test]
    fn excludes_works_outside_both_categories() {
        let g = load_str(FIXTURE).0;
        // Neither work is in Tech or Weather, so nothing matches.
        assert!(run(&g, "http://cat/Tech", "http://cat/Weather", 100).is_empty());
    }
}

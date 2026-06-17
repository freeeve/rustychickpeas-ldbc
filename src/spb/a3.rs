//! SPB advanced **q3** — creative works grouped by the minute-of-hour of their
//! `dateModified`, within a time window.
//!
//! Hand translation of `advanced/aggregation_standard/query3.txt`:
//! ```sparql
//! SELECT ?minute (COUNT(*) AS ?count) WHERE {
//!   ?creativeWork a cwork:CreativeWork ;
//!     cwork:dateModified ?dateModified .
//!   FILTER (?dateModified > {{{cwStartDateTime}}} && ?dateModified < {{{cwEndDateTime}}}) .
//!   BIND (MINUTES(?dateModified) AS ?minute) .
//! } GROUP BY ?minute ORDER BY DESC(?count)
//! ```
//!
//! The work type is pinned by the materialized `CreativeWork` super-class (works
//! are typed as `BlogPost`/`NewsItem`/… subclasses, which the loader
//! forward-chains). `MINUTES(?dateModified)` is the 0–59 minute field at chars
//! 14..16 of the fixed `YYYY-MM-DDTHH:MM:SS` lexical form; the window bounds are
//! exclusive lexicographic comparisons. Counts each minute, ordered by count
//! descending then minute.

use std::collections::HashMap;

use rustychickpeas_core::GraphSnapshot;

/// Creative works whose `dateModified` is strictly within `(after, before)`,
/// counted by the minute-of-hour component (the SPARQL `MINUTES`). Returned as
/// `(minute, count)` with `minute` a plain decimal string (no leading zero),
/// ordered by count descending then minute.
pub fn run(g: &GraphSnapshot, after: &str, before: &str) -> Vec<(String, usize)> {
    let Some(works) = g.nodes_with_label("CreativeWork") else {
        return Vec::new();
    };
    let mut counts: HashMap<u32, usize> = HashMap::new();
    for w in works.iter() {
        let Some(dt) = g.str_prop(w, "dateModified") else {
            continue;
        };
        if !(dt > after && dt < before) {
            continue;
        }
        // MINUTES(?dateModified): the MM field at chars 14..16 of YYYY-MM-DDTHH:MM:SS.
        let Some(minute) = dt.get(14..16).and_then(|m| m.parse::<u32>().ok()) else {
            continue;
        };
        *counts.entry(minute).or_default() += 1;
    }
    let mut rows: Vec<(u32, usize)> = counts.into_iter().collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    rows.into_iter().map(|(m, n)| (m.to_string(), n)).collect()
}

#[cfg(test)]
mod tests {
    use super::super::loader::load_str;
    use super::*;

    // TBox (BlogPost subClassOf CreativeWork) + four in-window works (two sharing
    // minute 30) and one work before the window.
    const FIXTURE: &str = r#"
<http://bbc/BlogPost> <http://www.w3.org/2000/01/rdf-schema#subClassOf> <http://bbc/CreativeWork> .

<http://ex/cw1> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/BlogPost> .
<http://ex/cw1> <http://bbc/dateModified> "2011-04-01T12:30:15.000Z"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/cw2> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/BlogPost> .
<http://ex/cw2> <http://bbc/dateModified> "2011-06-02T08:30:45.000Z"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/cw3> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/BlogPost> .
<http://ex/cw3> <http://bbc/dateModified> "2011-07-03T09:04:00.000Z"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/cw4> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/BlogPost> .
<http://ex/cw4> <http://bbc/dateModified> "2011-08-04T10:15:00.000Z"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/cw5> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/BlogPost> .
<http://ex/cw5> <http://bbc/dateModified> "2010-01-01T00:30:00.000Z"^^<http://www.w3.org/2001/XMLSchema#dateTime> .
"#;

    #[test]
    fn counts_by_minute_in_window() {
        let g = load_str(FIXTURE).0;
        // In-window: cw1/cw2 at minute 30 (count 2), cw3 at minute 4, cw4 at
        // minute 15. cw5 (minute 30) is before the window, so it does not count.
        // Ordered by count desc then minute: 30, then 4, then 15. The minute "04"
        // renders as "4" (no leading zero).
        let rows = run(&g, "2011-01-01", "2012-01-01");
        assert_eq!(
            rows,
            vec![("30".to_string(), 2), ("4".to_string(), 1), ("15".to_string(), 1)]
        );
    }
}

//! SPB advanced **q4** — how many creative works of each concrete subtype were
//! modified inside a date window, ranked by count.
//!
//! Hand translation of `advanced/aggregation_standard/query4.txt`:
//! ```sparql
//! SELECT ?type (COUNT(*) AS ?count) WHERE {
//!   ?creativeWork a ?type ; a cwork:CreativeWork ;
//!     cwork:dateModified ?dm .
//!   FILTER (?type != cwork:CreativeWork &&
//!           ?dm > {{{cwStartDateTime}}} && ?dm < {{{cwEndDateTime}}}) .
//! } GROUP BY ?type ORDER BY DESC(?count) LIMIT 10
//! ```
//!
//! `?creativeWork a ?type` with `?type != cwork:CreativeWork` ranges over the
//! concrete subclasses the data instantiates — `BlogPost` / `NewsItem` /
//! `Programme` (the loader labels every work with its subtype as well as the
//! materialized `CreativeWork` supertype). We count, per subtype label, the works
//! whose `dateModified` is strictly within `(after, before)` (exclusive,
//! lexicographic ISO-date comparison) and return `(subtype_local_name, count)`.
//! As under `GROUP BY`, subtypes with no in-window work are absent.

use rustychickpeas_core::GraphSnapshot;

use crate::props::{top_k_by_key, PropExt};

/// The concrete `CreativeWork` subclasses the SPB data instantiates; `?type !=
/// cwork:CreativeWork` in the template ranges over exactly these (the loader
/// forward-chains the `CreativeWork` supertype onto each subtyped work).
const SUBTYPES: [&str; 3] = ["BlogPost", "NewsItem", "Programme"];

/// Creative-work subtypes ranked by how many of their works have `dateModified`
/// strictly within `(after, before)`. Returned as `(subtype_local_name, count)`
/// ordered by count descending then name ascending, truncated to `limit`.
pub fn run(g: &GraphSnapshot, after: &str, before: &str, limit: usize) -> Vec<(String, usize)> {
    let count_in_window = |label: &str| {
        g.nodes_with_label(label).map_or(0, |works| {
            works
                .iter()
                .filter(|&w| {
                    g.prop(w, "dateModified").str()
                        .filter(|s| !s.is_empty())
                        .is_some_and(|dm| dm > after && dm < before)
                })
                .count()
        })
    };
    let rows = SUBTYPES
        .iter()
        .map(|&label| (label.to_string(), count_in_window(label)))
        .filter(|&(_, n)| n > 0);
    top_k_by_key(rows, limit)
}

#[cfg(test)]
mod tests {
    use super::super::loader::load_str;
    use super::*;

    // Two BlogPosts (both in window) and two NewsItems (one in, one out); no
    // Programme. Each work is typed with its subtype plus the CreativeWork
    // supertype, as the extract materializes it.
    const FIXTURE: &str = r#"
<http://ex/bp1> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/BlogPost> .
<http://ex/bp1> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/CreativeWork> .
<http://ex/bp1> <http://bbc/dateModified> "2011-04-01T00:00:00.000Z"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/bp2> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/BlogPost> .
<http://ex/bp2> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/CreativeWork> .
<http://ex/bp2> <http://bbc/dateModified> "2011-04-15T00:00:00.000Z"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/ni1> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/NewsItem> .
<http://ex/ni1> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/CreativeWork> .
<http://ex/ni1> <http://bbc/dateModified> "2011-04-10T00:00:00.000Z"^^<http://www.w3.org/2001/XMLSchema#dateTime> .

<http://ex/ni2> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/NewsItem> .
<http://ex/ni2> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/CreativeWork> .
<http://ex/ni2> <http://bbc/dateModified> "2011-09-01T00:00:00.000Z"^^<http://www.w3.org/2001/XMLSchema#dateTime> .
"#;

    #[test]
    fn counts_subtypes_in_window_ranked() {
        let g = load_str(FIXTURE).0;
        let rows = run(&g, "2011-03-01", "2011-06-01", 10);
        // bp1+bp2 in window -> BlogPost 2; ni1 in, ni2 (Sept) out -> NewsItem 1;
        // Programme has no work, so it never appears.
        assert_eq!(
            rows,
            vec![("BlogPost".to_string(), 2), ("NewsItem".to_string(), 1)]
        );
    }
}

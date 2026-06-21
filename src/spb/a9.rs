//! SPB advanced **q9** — the largest number of `mentions` carried by any single
//! creative work.
//!
//! Hand translation of `advanced/aggregation_standard/query9.txt`:
//! ```sparql
//! SELECT ?creativeWork (COUNT(?mention) AS ?mentions) WHERE {
//!   ?creativeWork a cwork:CreativeWork ;
//!     cwork:mentions ?mention .
//! } GROUP BY ?creativeWork ORDER BY DESC(?mentions) LIMIT 1
//! ```
//!
//! The work type is pinned by the materialized `CreativeWork` super-class (works
//! are typed as `BlogPost`/`NewsItem`/… subclasses, which the loader
//! forward-chains). For each creative work we count its OUTGOING `mentions`
//! rels and return the maximum (the `ORDER BY DESC … LIMIT 1` count), 0 when no
//! work mentions anything.

use rustychickpeas_core::{Direction, GraphSnapshot};

/// The maximum number of outgoing `mentions` rels on any single creative work
/// (0 when no work has any).
pub fn run(g: &GraphSnapshot) -> usize {
    let Some(works) = g.nodes_with_label("CreativeWork") else {
        return 0;
    };
    works
        .iter()
        .map(|w| {
            g.neighbors_by_type(w, Direction::Outgoing, "mentions")
                .count()
        })
        .max()
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::super::loader::load_str;
    use super::*;

    // TBox (BlogPost subClassOf CreativeWork) + three works carrying 0, 1 and 3
    // mentions respectively.
    const FIXTURE: &str = r#"
<http://bbc/BlogPost> <http://www.w3.org/2000/01/rdf-schema#subClassOf> <http://bbc/CreativeWork> .

<http://ex/cw1> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/BlogPost> .

<http://ex/cw2> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/BlogPost> .
<http://ex/cw2> <http://bbc/mentions> <http://ex/f1> .

<http://ex/cw3> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/BlogPost> .
<http://ex/cw3> <http://bbc/mentions> <http://ex/f1> .
<http://ex/cw3> <http://bbc/mentions> <http://ex/f2> .
<http://ex/cw3> <http://bbc/mentions> <http://ex/f3> .
"#;

    #[test]
    fn max_mentions_across_works() {
        let g = load_str(FIXTURE).0;
        assert_eq!(run(&g), 3);
    }

    #[test]
    fn no_mentions_is_zero() {
        // A lone CreativeWork with no mentions: max over {0} is 0.
        let doc = "<http://ex/cw1> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/CreativeWork> .";
        let g = load_str(doc).0;
        assert_eq!(run(&g), 0);
    }
}

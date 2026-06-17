//! SPB advanced **q2** — a single creative work decorated with its
//! `CreativeWork` subtype(s).
//!
//! Hand translation of `advanced/aggregation_standard/query2.txt`:
//! ```sparql
//! CONSTRUCT { ?cWork a ?type ; cwork:title ?title } WHERE {
//!   ?cWork a cwork:CreativeWork ; a ?type ; cwork:title ?title .
//!   ?type rdfs:subClassOf cwork:CreativeWork .
//!   OPTIONAL { ?cWork cwork:description ?description }
//! }
//! ```
//!
//! The template pins one `?cWork` by uri; the result keys on the `?type` values
//! that are `rdfs:subClassOf cwork:CreativeWork` — the work's concrete subtypes
//! (`BlogPost` / `NewsItem` / `Programme`). The loader forward-chains the TBox,
//! so every such work is labelled both `CreativeWork` and its subtype(s); we
//! read those labels off the resolved node rather than walking the subclass
//! hierarchy per query. A work that is missing or has no `title` constructs
//! nothing.

use rustychickpeas_core::GraphSnapshot;

use super::queries::{has_label, node_by_uri};

/// The `CreativeWork` subtype local names carried by the work at `cw_uri`, sorted.
/// Empty when the uri resolves to no titled `CreativeWork` — the subtypes are the
/// materialized `rdfs:subClassOf cwork:CreativeWork` labels.
pub fn run(g: &GraphSnapshot, cw_uri: &str) -> Vec<String> {
    let Some(work) = node_by_uri(g, cw_uri) else {
        return Vec::new();
    };
    if !has_label(g, work, "CreativeWork") {
        return Vec::new();
    }
    if g.str_prop(work, "title").is_none() {
        return Vec::new();
    }
    let mut subtypes: Vec<String> = ["BlogPost", "NewsItem", "Programme"]
        .into_iter()
        .filter(|&lbl| has_label(g, work, lbl))
        .map(str::to_string)
        .collect();
    subtypes.sort();
    subtypes
}

#[cfg(test)]
mod tests {
    use super::super::loader::load_str;
    use super::*;

    // TBox (BlogPost subClassOf CreativeWork) + a titled BlogPost and an untitled
    // one; both are typed as BlogPost + CreativeWork directly.
    const FIXTURE: &str = r#"
<http://bbc/BlogPost> <http://www.w3.org/2000/01/rdf-schema#subClassOf> <http://bbc/CreativeWork> .

<http://ex/cw1> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/BlogPost> .
<http://ex/cw1> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/CreativeWork> .
<http://ex/cw1> <http://bbc/title> "Breaking news" .

<http://ex/cw2> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/BlogPost> .
<http://ex/cw2> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/CreativeWork> .
"#;

    #[test]
    fn titled_work_yields_its_subtype() {
        let g = load_str(FIXTURE).0;
        assert_eq!(run(&g, "http://ex/cw1"), vec!["BlogPost".to_string()]);
    }

    #[test]
    fn untitled_or_missing_work_yields_nothing() {
        let g = load_str(FIXTURE).0;
        assert!(run(&g, "http://ex/cw2").is_empty()); // present but untitled
        assert!(run(&g, "http://ex/none").is_empty()); // unknown uri
    }
}

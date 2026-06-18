//! SPB basic **q2** — "retrieve the properties of a concrete creative work".
//!
//! The official SPARQL (aggregation/query2.txt) is a `CONSTRUCT` that pins a
//! single resource with `FILTER (?creativeWork = {{{cwUri}}})` and describes it:
//!
//! ```text
//! ?creativeWork a cwork:CreativeWork ; a ?type ; cwork:title ?title .
//! ?type rdfs:subClassOf cwork:CreativeWork .
//! OPTIONAL { ?creativeWork cwork:dateCreated  ?dateCreated  . }
//! OPTIONAL { ?creativeWork cwork:dateModified ?dateModified . }
//! OPTIONAL { ?creativeWork cwork:about [ ldbcspb:prefLabel ?aboutPrefLabel ] . }
//! OPTIONAL { ?creativeWork bbc:primaryContentOf ?pco . ?pco bbc:webDocumentType ?t . }
//! ```
//!
//! In plain English: *look up the creative work with this exact uri, and only if
//! it is a `CreativeWork` that has a `title`, return it so the caller can read
//! off its properties (title, dateCreated/Modified) and `about`/`mentions`
//! neighbours.* The lookup is the whole query — the `CONSTRUCT` body is just the
//! described node's own columns and out-edges.
//!
//! Two parts of the SPARQL drop out by design:
//!   * `?type rdfs:subClassOf cwork:CreativeWork` is the redundant pattern the
//!     query's own header tells the optimizer to eliminate (and we have no RDFS
//!     reasoning); membership in the `CreativeWork` label already implies it.
//!   * the four `OPTIONAL`s never reject a row, so they are not part of the
//!     match — they are the describe payload, read by the caller via `pstr` /
//!     `neighbors_by_type` on the returned node.

use rustychickpeas_core::GraphSnapshot;

use crate::props::PropExt;

/// Resolve SPB basic q2: the `CreativeWork` whose `uri` equals `cw_uri`, but only
/// if it carries the required (non-OPTIONAL) `title`. Returns its node id, or
/// `None` for an unknown uri, a non-`CreativeWork` resource, or a titleless work.
///
/// The caller "describes" the node by reading its properties
/// (`g.prop(n, k).str()` for `title` / `dateCreated` / `dateModified`) and its
/// `about` / `mentions`
/// edges, mirroring the SPARQL `CONSTRUCT` block.
pub fn run(g: &GraphSnapshot, cw_uri: &str) -> Option<u32> {
    let node = g
        .nodes_with_property("CreativeWork", "uri", cw_uri)
        .and_then(|ns| ns.iter().next())?;
    // The non-OPTIONAL `cwork:title ?title` pattern: no title -> no solution.
    g.prop(node, "title").str().map(|_| node)
}

#[cfg(test)]
mod tests {
    use super::super::loader::load_str;
    use super::*;

    const FIXTURE: &str = r#"
<http://ex/cw1> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw1> <http://www.bbc.co.uk/ontologies/creativework/title> "London derby" .
<http://ex/cw1> <http://www.bbc.co.uk/ontologies/creativework/dateCreated> "2011-08-15T08:00:00.000Z" .
<http://ex/cw1> <http://www.bbc.co.uk/ontologies/creativework/dateModified> "2011-08-15T12:30:00.000Z" .
<http://ex/cw1> <http://www.bbc.co.uk/ontologies/creativework/mentions> <http://sws.geonames.org/london> .

<http://ex/cw2> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .

<http://sws.geonames.org/london> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.geonames.org/ontology#Feature> .
<http://sws.geonames.org/london> <http://www.geonames.org/ontology#name> "London" .
"#;

    #[test]
    fn looks_up_a_concrete_creative_work_by_uri() {
        let g = load_str(FIXTURE).0;

        // Known creative work with a title -> Some, and its describe payload is
        // readable off the returned node.
        let node = run(&g, "http://ex/cw1").expect("cw1 should resolve");
        assert_eq!(g.prop(node, "title").str(), Some("London derby"));
        assert_eq!(
            g.prop(node, "dateModified").str(),
            Some("2011-08-15T12:30:00.000Z")
        );

        // Unknown uri -> None.
        assert_eq!(run(&g, "http://ex/missing"), None);

        // A Feature with that uri must not satisfy `a cwork:CreativeWork`.
        assert_eq!(run(&g, "http://sws.geonames.org/london"), None);

        // The required (non-OPTIONAL) `cwork:title` pattern: a titleless
        // CreativeWork yields no solution.
        assert_eq!(run(&g, "http://ex/cw2"), None);
    }
}

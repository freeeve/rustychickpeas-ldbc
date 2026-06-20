//! SPB advanced **q14** — a "star" query: creative works carrying the full
//! required property star (tag, category, dateModified, thumbnail, audience,
//! primaryFormat, and a primaryContentOf web document), restricted to a pinned
//! primaryFormat and the web document's type, newest first.
//!
//! Hand translation of `advanced/aggregation_standard/query14.txt` (no SPARQL
//! engine):
//! ```sparql
//! SELECT ?thing ?about ?mentions ?category ?dateModified ?thumbnail ?primaryFormat WHERE {
//!   ?thing rdf:type cwork:CreativeWork .
//!   ?thing cwork:tag ?tag .
//!   ?thing cwork:category ?category .
//!   ?thing cwork:dateModified ?dateModified .
//!   ?thing cwork:thumbnail ?thumbnail .
//!   ?thing cwork:audience ?audience .
//!   ?thing cwork:primaryFormat ?primaryFormat .
//!   ?thing bbc:primaryContentOf ?primaryContent .
//!   ?primaryContent bbc:webDocumentType ?webdoc .
//!   OPTIONAL { ?thing cwork:mentions ?mentions . ?thing cwork:about ?about . }
//!   OPTIONAL { ?thing cwork:audience {{{cwAudienceType}}} . }
//!   FILTER ( (?audience = {{{cwAudienceType}}}) && (?webdoc = {{{cwWebDocumentType}}})
//!            && ((?primaryFormat = {{{cwPrimaryFormat}}}) || (?primaryFormat = {{{cwPrimaryFormat}}})) )
//! } ORDER BY DESC(?dateModified) LIMIT 200
//! ```
//!
//! `cwork:tag` is the RDFS super-property of `about`/`mentions` (the loader
//! forward-chains it, so the topic links materialize a `tag` rel — same as q8);
//! the remaining required patterns demand ≥1 `category`/`thumbnail`/`audience`
//! rel and a non-empty `dateModified`. The pinned facets are exact-value: an
//! outgoing `primaryFormat` rel to `primary_format_uri`, and a `primaryContentOf`
//! web document whose `webDocumentType` targets `web_doc_type`. Both
//! `primaryFormat` and `webDocumentType` carry IRI objects in the data
//! (`bbc:webDocumentType <…/Mobile>`), so the loader stores them as rels to a
//! node bearing that `uri` — we match them by resolving the target `uri` to its
//! node and traversing to it, not a literal-property read. The template additionally pins a specific
//! `audience` (in the FILTER and a second OPTIONAL); this batch's signature leaves
//! the audience unbound, so we require only that an `audience` rel is present.
//! Results are ordered by `dateModified` (ISO-8601, hence lexicographic)
//! descending, id tie-break, truncated to `limit` (the template's `LIMIT 200`).

use rustychickpeas_core::{Direction, GraphSnapshot};

use super::queries::node_by_uri;
use crate::props::top_k_by_key;

/// Whether `work` binds every required rel of the q14 star: ≥1 `tag`
/// (about∪mentions), `category`, `thumbnail` and `audience` rel.
fn has_required_star(g: &GraphSnapshot, work: u32) -> bool {
    g.has_rel(work, Direction::Outgoing, "tag")
        && g.has_rel(work, Direction::Outgoing, "category")
        && g.has_rel(work, Direction::Outgoing, "thumbnail")
        && g.has_rel(work, Direction::Outgoing, "audience")
}

/// q14: creative works satisfying the full required star (≥1 `tag`/`category`/
/// `thumbnail`/`audience` rel, a non-empty `dateModified`, a `primaryFormat` rel
/// to `primary_format_uri`, and a `primaryContentOf` web document of type
/// `web_doc_type`), ordered by `dateModified` descending then id, truncated to
/// `limit`.
pub fn run(
    g: &GraphSnapshot,
    primary_format_uri: &str,
    web_doc_type: &str,
    limit: usize,
) -> Vec<u32> {
    // Resolve the two pinned facet targets to node ids once (a `Facet`-labelled
    // uri lookup), so the filters are id comparisons, not per-rel uri reads.
    let (Some(works), Some(pf), Some(wdt)) = (
        g.nodes_with_label("CreativeWork"),
        node_by_uri(g, primary_format_uri),
        node_by_uri(g, web_doc_type),
    ) else {
        return Vec::new();
    };
    let rows: Vec<(u32, &str)> = works
        .iter()
        .filter(|&w| has_required_star(g, w))
        .filter(|&w| {
            g.neighbors_by_type(w, Direction::Outgoing, "primaryFormat")
                .any(|t| t == pf)
        })
        .filter(|&w| {
            g.neighbors_by_type(w, Direction::Outgoing, "primaryContentOf")
                .any(|pc| {
                    g.neighbors_by_type(pc, Direction::Outgoing, "webDocumentType")
                        .any(|t| t == wdt)
                })
        })
        // `cwork:dateModified ?dateModified` is required and is the ORDER BY key;
        // a dense string property missing on a node reads back as Some(""), so
        // treat empty as absent. Carry the value to sort without re-lookup.
        .filter_map(|w| g.prop_str(w, "dateModified").map(|d| (w, d)))
        .collect();
    top_k_by_key(rows, limit)
        .into_iter()
        .map(|(w, _)| w)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::loader::load_str;
    use super::*;
    use crate::props::PropExt;

    const VIDEO: &str = "http://www.bbc.co.uk/ontologies/creativework/VideoFormat";
    const MOBILE: &str = "http://www.bbc.co.uk/ontologies/bbc/Mobile";

    // TBox (about/mentions subPropertyOf tag, so the topic links materialize a tag
    // rel) plus five CreativeWorks: two fully-specified Video/Mobile matches
    // (differing dateModified, to exercise the DESC ordering) and three that each
    // drop one required/filtered pattern — missing thumbnail, wrong primaryFormat,
    // wrong webDocumentType — and so must be excluded.
    const FIXTURE: &str = r#"
<http://www.bbc.co.uk/ontologies/creativework/about> <http://www.w3.org/2000/01/rdf-schema#subPropertyOf> <http://www.bbc.co.uk/ontologies/creativework/tag> .
<http://www.bbc.co.uk/ontologies/creativework/mentions> <http://www.w3.org/2000/01/rdf-schema#subPropertyOf> <http://www.bbc.co.uk/ontologies/creativework/tag> .

<http://ex/cw-new> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw-new> <http://www.bbc.co.uk/ontologies/creativework/title> "Newest" .
<http://ex/cw-new> <http://www.bbc.co.uk/ontologies/creativework/category> <http://www.bbc.co.uk/category/Politics> .
<http://ex/cw-new> <http://www.bbc.co.uk/ontologies/creativework/dateModified> "2024-06-01T12:00:00.000Z"^^<http://www.w3.org/2001/XMLSchema#dateTime> .
<http://ex/cw-new> <http://www.bbc.co.uk/ontologies/creativework/thumbnail> <http://www.bbc.co.uk/thumbnail/1> .
<http://ex/cw-new> <http://www.bbc.co.uk/ontologies/creativework/audience> <http://www.bbc.co.uk/ontologies/creativework/NationalAudience> .
<http://ex/cw-new> <http://www.bbc.co.uk/ontologies/creativework/primaryFormat> <http://www.bbc.co.uk/ontologies/creativework/VideoFormat> .
<http://ex/cw-new> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/Policy> .
<http://ex/cw-new> <http://www.bbc.co.uk/ontologies/bbc/primaryContentOf> <http://ex/doc-new> .
<http://ex/doc-new> <http://www.bbc.co.uk/ontologies/bbc/webDocumentType> <http://www.bbc.co.uk/ontologies/bbc/Mobile> .

<http://ex/cw-old> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw-old> <http://www.bbc.co.uk/ontologies/creativework/title> "Oldest" .
<http://ex/cw-old> <http://www.bbc.co.uk/ontologies/creativework/category> <http://www.bbc.co.uk/category/Politics> .
<http://ex/cw-old> <http://www.bbc.co.uk/ontologies/creativework/dateModified> "2024-01-01T12:00:00.000Z"^^<http://www.w3.org/2001/XMLSchema#dateTime> .
<http://ex/cw-old> <http://www.bbc.co.uk/ontologies/creativework/thumbnail> <http://www.bbc.co.uk/thumbnail/2> .
<http://ex/cw-old> <http://www.bbc.co.uk/ontologies/creativework/audience> <http://www.bbc.co.uk/ontologies/creativework/NationalAudience> .
<http://ex/cw-old> <http://www.bbc.co.uk/ontologies/creativework/primaryFormat> <http://www.bbc.co.uk/ontologies/creativework/VideoFormat> .
<http://ex/cw-old> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/Policy> .
<http://ex/cw-old> <http://www.bbc.co.uk/ontologies/bbc/primaryContentOf> <http://ex/doc-old> .
<http://ex/doc-old> <http://www.bbc.co.uk/ontologies/bbc/webDocumentType> <http://www.bbc.co.uk/ontologies/bbc/Mobile> .

<http://ex/cw-nothumb> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw-nothumb> <http://www.bbc.co.uk/ontologies/creativework/title> "No thumbnail" .
<http://ex/cw-nothumb> <http://www.bbc.co.uk/ontologies/creativework/category> <http://www.bbc.co.uk/category/Politics> .
<http://ex/cw-nothumb> <http://www.bbc.co.uk/ontologies/creativework/dateModified> "2024-05-01T12:00:00.000Z"^^<http://www.w3.org/2001/XMLSchema#dateTime> .
<http://ex/cw-nothumb> <http://www.bbc.co.uk/ontologies/creativework/audience> <http://www.bbc.co.uk/ontologies/creativework/NationalAudience> .
<http://ex/cw-nothumb> <http://www.bbc.co.uk/ontologies/creativework/primaryFormat> <http://www.bbc.co.uk/ontologies/creativework/VideoFormat> .
<http://ex/cw-nothumb> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/Policy> .
<http://ex/cw-nothumb> <http://www.bbc.co.uk/ontologies/bbc/primaryContentOf> <http://ex/doc-nothumb> .
<http://ex/doc-nothumb> <http://www.bbc.co.uk/ontologies/bbc/webDocumentType> <http://www.bbc.co.uk/ontologies/bbc/Mobile> .

<http://ex/cw-wrongfmt> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw-wrongfmt> <http://www.bbc.co.uk/ontologies/creativework/title> "Wrong format" .
<http://ex/cw-wrongfmt> <http://www.bbc.co.uk/ontologies/creativework/category> <http://www.bbc.co.uk/category/Politics> .
<http://ex/cw-wrongfmt> <http://www.bbc.co.uk/ontologies/creativework/dateModified> "2024-05-01T12:00:00.000Z"^^<http://www.w3.org/2001/XMLSchema#dateTime> .
<http://ex/cw-wrongfmt> <http://www.bbc.co.uk/ontologies/creativework/thumbnail> <http://www.bbc.co.uk/thumbnail/3> .
<http://ex/cw-wrongfmt> <http://www.bbc.co.uk/ontologies/creativework/audience> <http://www.bbc.co.uk/ontologies/creativework/NationalAudience> .
<http://ex/cw-wrongfmt> <http://www.bbc.co.uk/ontologies/creativework/primaryFormat> <http://www.bbc.co.uk/ontologies/creativework/TextualFormat> .
<http://ex/cw-wrongfmt> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/Policy> .
<http://ex/cw-wrongfmt> <http://www.bbc.co.uk/ontologies/bbc/primaryContentOf> <http://ex/doc-wrongfmt> .
<http://ex/doc-wrongfmt> <http://www.bbc.co.uk/ontologies/bbc/webDocumentType> <http://www.bbc.co.uk/ontologies/bbc/Mobile> .

<http://ex/cw-wrongdoc> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw-wrongdoc> <http://www.bbc.co.uk/ontologies/creativework/title> "Wrong web document" .
<http://ex/cw-wrongdoc> <http://www.bbc.co.uk/ontologies/creativework/category> <http://www.bbc.co.uk/category/Politics> .
<http://ex/cw-wrongdoc> <http://www.bbc.co.uk/ontologies/creativework/dateModified> "2024-05-01T12:00:00.000Z"^^<http://www.w3.org/2001/XMLSchema#dateTime> .
<http://ex/cw-wrongdoc> <http://www.bbc.co.uk/ontologies/creativework/thumbnail> <http://www.bbc.co.uk/thumbnail/4> .
<http://ex/cw-wrongdoc> <http://www.bbc.co.uk/ontologies/creativework/audience> <http://www.bbc.co.uk/ontologies/creativework/NationalAudience> .
<http://ex/cw-wrongdoc> <http://www.bbc.co.uk/ontologies/creativework/primaryFormat> <http://www.bbc.co.uk/ontologies/creativework/VideoFormat> .
<http://ex/cw-wrongdoc> <http://www.bbc.co.uk/ontologies/creativework/about> <http://dbpedia.org/resource/Policy> .
<http://ex/cw-wrongdoc> <http://www.bbc.co.uk/ontologies/bbc/primaryContentOf> <http://ex/doc-wrongdoc> .
<http://ex/doc-wrongdoc> <http://www.bbc.co.uk/ontologies/bbc/webDocumentType> <http://www.bbc.co.uk/ontologies/bbc/HighWeb> .
"#;

    fn uris(g: &GraphSnapshot, works: &[u32]) -> Vec<String> {
        works
            .iter()
            .map(|&w| g.prop(w, "uri").str().unwrap_or("?").to_string())
            .collect()
    }

    #[test]
    fn full_star_filtered_and_ordered_desc() {
        let g = load_str(FIXTURE).0;
        let out = run(&g, VIDEO, MOBILE, 100);
        // Only the two fully-specified Video/Mobile works survive, newest first;
        // the missing-thumbnail, wrong-format and wrong-web-document works drop out.
        assert_eq!(uris(&g, &out), ["http://ex/cw-new", "http://ex/cw-old"]);
    }

    #[test]
    fn limit_truncates_after_date_order() {
        let g = load_str(FIXTURE).0;
        // LIMIT 1 keeps the newest of the two matches.
        assert_eq!(uris(&g, &run(&g, VIDEO, MOBILE, 1)), ["http://ex/cw-new"]);
    }

    #[test]
    fn unmatched_format_yields_empty() {
        let g = load_str(FIXTURE).0;
        // A primaryFormat nobody carries -> no matches.
        let out = run(
            &g,
            "http://www.bbc.co.uk/ontologies/creativework/AudioFormat",
            MOBILE,
            100,
        );
        assert!(out.is_empty());
    }
}

//! SPB advanced **q15** — creative works that are *about* and *mention* things of
//! the **same entity type**, whose title carries an uncommon word ("policy").
//!
//! Hand translation of `advanced/aggregation_standard/query15.txt` (no SPARQL
//! engine):
//! ```sparql
//! SELECT DISTINCT ?thing ?about ?mentions ?entityType ?category ?title WHERE {
//!   ?thing rdf:type cwork:CreativeWork .
//!   ?thing rdf:type ?class . ?class rdfs:subClassOf cwork:CreativeWork .
//!   ?thing cwork:about ?about . ?thing cwork:mentions ?mentions .
//!   ?mentions rdf:type ?entityType . ?about rdf:type ?entityType .
//!   ?thing cwork:category ?category . ?thing cwork:title ?title .
//!   FILTER (CONTAINS (?title, "policy")) .
//!   OPTIONAL { ?thing cwork:audience {{{cwAudienceType}}} . }
//! } ORDER BY ?about LIMIT {{{randomLimit}}}
//! ```
//!
//! Vocabulary mapping (no SPARQL / RDFS engine):
//!   * `?thing a cwork:CreativeWork ; a ?class . ?class rdfs:subClassOf
//!     cwork:CreativeWork` — the generator types every work with a concrete
//!     subclass (BlogPost/NewsItem/…) that the loader forward-chains to
//!     `CreativeWork`, so this clause holds for every work; we scan the
//!     `CreativeWork` full-text index for the title FILTER.
//!   * `FILTER (CONTAINS (?title, word))` — the core whole-word `full_text_search` over `title`.
//!   * `?about a ?entityType . ?mentions a ?entityType` — an about-target and a
//!     mentions-target must share a type. SPB's about-targets are dbpedia
//!     `Company`/`Event` and its mentions-targets are geonames `Feature`s; the
//!     loader forward-chains `coreconcepts:Thing` onto all of them via
//!     `rdfs:subClassOf`, so `Thing` is the only entity type an about- and a
//!     mentions-target can share. We require one of each carrying it.
//!   * `cwork:category ?category` — at least one outgoing `category` edge.
//!   * the OPTIONAL audience and the projected `?about`/`?mentions`/`?entityType`/
//!     `?category` columns only fan each qualifying DISTINCT work out into a cross
//!     product; the meaningful identity is the work, so `run` returns the
//!     qualifying work ids (sorted by id, the template's `ORDER BY ?about` being a
//!     presentation order over those fanned-out columns).

use rustychickpeas_core::{Direction, GraphSnapshot};

use super::queries::has_label;

/// The materialized universal entity type. SPB about-targets (`Company`/`Event`)
/// and mentions-targets (geonames `Feature`) are each forward-chained to
/// `coreconcepts:Thing` (`rdfs:subClassOf`), so it is the one `?entityType` an
/// about- and a mentions-target can share.
const SHARED_ENTITY_TYPE: &str = "Thing";

/// Whether `work` has an about-target and a mentions-target that share the
/// `Thing` entity type — the q15 `?about a ?entityType . ?mentions a ?entityType`
/// join, whose only solution on the SPB vocabulary binds `?entityType` to `Thing`.
fn about_and_mentions_share_type(g: &GraphSnapshot, work: u32) -> bool {
    let about_thing = g
        .neighbors_by_type(work, Direction::Outgoing, "about")
        .any(|a| has_label(g, a, SHARED_ENTITY_TYPE));
    let mentions_thing = g
        .neighbors_by_type(work, Direction::Outgoing, "mentions")
        .any(|m| has_label(g, m, SHARED_ENTITY_TYPE));
    about_thing && mentions_thing
}

/// Run SPB advanced q15: DISTINCT creative works whose `title` matches `word`
/// (full-text), that carry a `category` edge and an about-/mentions-target pair
/// sharing the `Thing` entity type, sorted by id and truncated to `limit` (the
/// template's `LIMIT {{{randomLimit}}}`).
pub fn run(g: &GraphSnapshot, word: &str, limit: usize) -> Vec<u32> {
    let mut out: Vec<u32> = g
        .full_text_search("CreativeWork", "title", word)
        .iter()
        .filter(|&w| {
            g.neighbors_by_type(w, Direction::Outgoing, "category")
                .next()
                .is_some()
                && about_and_mentions_share_type(g, w)
        })
        .collect();
    out.sort_unstable();
    out.truncate(limit);
    out
}

#[cfg(test)]
mod tests {
    use super::super::loader::load_str;
    use super::*;
    use crate::props::pstr;

    // TBox: a work subtype reaches `CreativeWork`; the about-target `Company` and
    // the mentions-target `Feature` each reach `coreconcepts:Thing` (the shared
    // `?entityType`); about/mentions are sub-properties of `tag`.
    // Instances (word "policy"):
    //   cw1 — about a Company + mentions a Feature (both Thing), category, "policy"
    //         title                                                    -> included
    //   cw2 — only an about (no mentions)                              -> excluded
    //   cw3 — about+mentions+category but title lacks "policy"         -> excluded
    //   cw4 — policy + about+mentions Things but no category           -> excluded
    //   cw5 — like cw1, a second qualifying work                       -> included
    const FIXTURE: &str = r#"
<http://bbc/BlogPost> <http://www.w3.org/2000/01/rdf-schema#subClassOf> <http://bbc/CreativeWork> .
<http://dbo/Company> <http://www.w3.org/2000/01/rdf-schema#subClassOf> <http://cc/Thing> .
<http://geo/Feature> <http://www.w3.org/2000/01/rdf-schema#subClassOf> <http://cc/Thing> .
<http://bbc/about> <http://www.w3.org/2000/01/rdf-schema#subPropertyOf> <http://bbc/tag> .
<http://bbc/mentions> <http://www.w3.org/2000/01/rdf-schema#subPropertyOf> <http://bbc/tag> .

<http://ex/Acme> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://dbo/Company> .
<http://ex/Globex> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://dbo/Company> .
<http://ex/London> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://geo/Feature> .

<http://ex/cw1> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/BlogPost> .
<http://ex/cw1> <http://bbc/title> "the new policy debate" .
<http://ex/cw1> <http://bbc/category> <http://cat/Politics> .
<http://ex/cw1> <http://bbc/about> <http://ex/Acme> .
<http://ex/cw1> <http://bbc/mentions> <http://ex/London> .

<http://ex/cw2> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/BlogPost> .
<http://ex/cw2> <http://bbc/title> "policy reform plan" .
<http://ex/cw2> <http://bbc/category> <http://cat/Politics> .
<http://ex/cw2> <http://bbc/about> <http://ex/Globex> .

<http://ex/cw3> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/BlogPost> .
<http://ex/cw3> <http://bbc/title> "the weather report" .
<http://ex/cw3> <http://bbc/category> <http://cat/Weather> .
<http://ex/cw3> <http://bbc/about> <http://ex/Acme> .
<http://ex/cw3> <http://bbc/mentions> <http://ex/London> .

<http://ex/cw4> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/BlogPost> .
<http://ex/cw4> <http://bbc/title> "a policy aside" .
<http://ex/cw4> <http://bbc/about> <http://ex/Acme> .
<http://ex/cw4> <http://bbc/mentions> <http://ex/London> .

<http://ex/cw5> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://bbc/BlogPost> .
<http://ex/cw5> <http://bbc/title> "second policy memo" .
<http://ex/cw5> <http://bbc/category> <http://cat/Politics> .
<http://ex/cw5> <http://bbc/about> <http://ex/Acme> .
<http://ex/cw5> <http://bbc/mentions> <http://ex/London> .
"#;

    fn uris(g: &GraphSnapshot, works: &[u32]) -> Vec<String> {
        works
            .iter()
            .map(|&w| pstr(g, w, "uri").unwrap_or("?").to_string())
            .collect()
    }

    #[test]
    fn keeps_policy_works_with_about_and_mentions_sharing_a_type() {
        let g = load_str(FIXTURE).0;
        let out = run(&g, "policy", 100);
        // cw1, cw5 qualify; cw2 (no mentions), cw3 (no "policy"), cw4 (no category) don't.
        assert_eq!(uris(&g, &out), ["http://ex/cw1", "http://ex/cw5"]);
    }

    #[test]
    fn limit_truncates_after_id_sort() {
        let g = load_str(FIXTURE).0;
        let out = run(&g, "policy", 1);
        // Lowest id wins the truncation; cw1 is declared before cw5.
        assert_eq!(uris(&g, &out), ["http://ex/cw1"]);
    }
}

//! SPB queries against the **real** SPB vocabulary, hand-translated from the
//! official SPARQL (no SPARQL engine) — the same approach the BI queries take
//! with Cypher.
//!
//! Vocabulary by RDF local name: creative works are `CreativeWork` (the extract
//! materializes this supertype; the data types them as `BlogPost`/`NewsItem`/…
//! subclasses), carrying `title` / `description` (full-text), `mentions` -> a
//! geonames `Feature` (wgs84 `lat` / `long`, geo), `about` -> entity,
//! `category`, `dateModified`. Full-text and geo use the core `fts` /
//! `geo_within_radius` indexes (tasks 011/012).
//!
//! This module implements the two queries the core features were built for —
//! SPB basic **q8** (full-text) and **q6** (geo) — plus their composition. The
//! remaining aggregation queries are tasks 015–048.

use std::collections::HashSet;

use rustychickpeas_core::{Direction, GraphSnapshot};

use crate::props::pstr;

/// Whether `node` carries the given label.
pub(crate) fn has_label(g: &GraphSnapshot, node: u32, label: &str) -> bool {
    g.nodes_with_label(label).is_some_and(|ns| ns.contains(node))
}

/// Find a node by its `uri` property: creative works, geonames features and
/// dbpedia `about`-targets (Company / Event), falling back to `Facet` for an
/// otherwise-untyped resource (category / audience / format / webDocument …).
/// Each lookup uses the cached `nodes_with_property` index. `None` for unknown.
pub(crate) fn node_by_uri(g: &GraphSnapshot, uri: &str) -> Option<u32> {
    ["CreativeWork", "Feature", "Company", "Event", "Facet"]
        .iter()
        .find_map(|lbl| g.nodes_with_property(lbl, "uri", uri).and_then(|ns| ns.iter().next()))
}

/// Display name: a creative work's `title`, or a feature's `name`.
pub fn name_of(g: &GraphSnapshot, node: u32) -> &str {
    pstr(g, node, "title").or_else(|| pstr(g, node, "name")).unwrap_or("?")
}

/// SPB basic **q8** (full-text): creative works whose `title` OR `description`
/// contains `word`. Boolean union of the core inverted index over both fields.
pub fn q8_fulltext(g: &GraphSnapshot, word: &str) -> Vec<u32> {
    let hits = &g.fts("CreativeWork", "description", word) | &g.fts("CreativeWork", "title", word);
    hits.iter().collect()
}

/// SPB basic **q6** (geo): creative works `mentions`-linked to a geonames
/// `Feature` within `km` great-circle of `(lat, lon)`. Core geo k-d tree, then
/// the reverse `mentions` traversal.
pub fn q6_geo(g: &GraphSnapshot, lat: f64, lon: f64, km: f64) -> Vec<u32> {
    let mut works: HashSet<u32> = HashSet::new();
    for f in g.geo_within_radius("Feature", "lat", "long", lat, lon, km).iter() {
        for w in g.neighbors_by_type(f, Direction::Incoming, "mentions") {
            if has_label(g, w, "CreativeWork") {
                works.insert(w);
            }
        }
    }
    let mut out: Vec<u32> = works.into_iter().collect();
    out.sort_unstable();
    out
}

/// **q6 ∩ q8** — creative works near `(lat, lon)` AND matching `word`. The
/// composition the two core indexes were built to enable.
pub fn q6_q8(g: &GraphSnapshot, lat: f64, lon: f64, km: f64, word: &str) -> Vec<u32> {
    let near: HashSet<u32> = q6_geo(g, lat, lon, km).into_iter().collect();
    let mut hits: Vec<u32> = q8_fulltext(g, word)
        .into_iter()
        .filter(|w| near.contains(w))
        .collect();
    hits.sort_unstable();
    hits
}

#[cfg(test)]
mod tests {
    use super::super::loader::load_str;
    use super::*;

    fn titles(g: &GraphSnapshot, works: &[u32]) -> Vec<String> {
        let mut t: Vec<String> = works.iter().map(|&w| name_of(g, w).to_string()).collect();
        t.sort();
        t
    }

    fn fixture() -> GraphSnapshot {
        load_str(&std::fs::read_to_string("samples/spb-sample.nt").unwrap()).0
    }

    #[test]
    fn q8_full_text_over_title_and_description() {
        let g = fixture();
        assert_eq!(titles(&g, &q8_fulltext(&g, "football")), ["London derby", "Paris Saint-Germain"]);
        assert_eq!(titles(&g, &q8_fulltext(&g, "tennis")), ["Wimbledon final"]);
        // matches title as well as description
        assert_eq!(titles(&g, &q8_fulltext(&g, "wimbledon")), ["Wimbledon final"]);
    }

    #[test]
    fn q6_geo_via_mentions() {
        let g = fixture();
        // within 50km of London -> the two works mentioning London
        assert_eq!(titles(&g, &q6_geo(&g, 51.5074, -0.1278, 50.0)), ["London derby", "Wimbledon final"]);
        // widen to cover Paris (~340km) -> all three
        assert_eq!(q6_geo(&g, 51.5074, -0.1278, 500.0).len(), 3);
    }

    #[test]
    fn q6_q8_composition() {
        let g = fixture();
        // near London AND 'tennis' -> Wimbledon only
        assert_eq!(titles(&g, &q6_q8(&g, 51.5074, -0.1278, 50.0, "tennis")), ["Wimbledon final"]);
        // near London AND 'football' -> London derby (the Paris football club is excluded)
        assert_eq!(titles(&g, &q6_q8(&g, 51.5074, -0.1278, 50.0, "football")), ["London derby"]);
    }
}

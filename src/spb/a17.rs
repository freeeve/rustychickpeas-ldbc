//! SPB advanced **q17** (geo drill-down, first hop): creative works that
//! `mentions` a geonames `Feature` lying inside a lat/long bounding box centered
//! on a reference point.
//!
//! Hand translation of advanced `aggregation_standard/query17.txt` (no SPARQL
//! engine). The SPARQL SELECTs DISTINCT creative works that `cwork:mentions` a
//! `geo-ont:Feature` carrying `geo:lat` / `geo:long`, under the box FILTER
//!   lat  in [refLat  - dev, refLat  + dev]
//!   long in [refLong - dev, refLong + dev]
//! i.e. a square of half-extent `deviation` (degrees) around
//! `(refLatitude, refLongtitude)`; each work must also carry
//! `cwork:dateModified` (a required triple in the SELECT).
//!
//! This is the drill-down's first hop — the `{{{orderBy}}}` / `{{{randomLimit}}}`
//! that pick the next iteration's reference work are runtime substitution params,
//! not part of the fixed graph pattern, so (like `queries::q6_geo`) we return the
//! full deduped match set sorted by id.
//!
//! Same shape as `q6_geo`, swapping the radius for the box: the core geo k-d tree
//! via `geo_within_bbox`, then the reverse `mentions` traversal.

use std::collections::HashSet;

use rustychickpeas_core::{Direction, GraphSnapshot};

use crate::props::PropExt;

/// Creative works `mentions`-linked to a geonames `Feature` inside the square box
/// of half-extent `deviation` degrees centered on `(ref_lat, ref_lon)` — the
/// SPARQL's `refLatitude` / `refLongtitude` / `refDeviation` — restricted to
/// works carrying a `dateModified`. Returns the matching CreativeWork ids,
/// deduped and sorted by id.
pub fn run(g: &GraphSnapshot, ref_lat: f64, ref_lon: f64, deviation: f64) -> Vec<u32> {
    let min = (ref_lat - deviation, ref_lon - deviation);
    let max = (ref_lat + deviation, ref_lon + deviation);

    let Some(cworks) = g.nodes_with_label("CreativeWork") else {
        return Vec::new();
    };
    let mut works: HashSet<u32> = HashSet::new();
    for f in g.geo_within_bbox("Feature", "lat", "long", min, max).iter() {
        for w in g.neighbors_in_set(f, Direction::Incoming, "mentions", cworks) {
            if g.prop(w, "dateModified").str().is_some() {
                works.insert(w);
            }
        }
    }
    let mut out: Vec<u32> = works.into_iter().collect();
    out.sort_unstable();
    out
}

#[cfg(test)]
mod tests {
    use super::super::loader::load_str;
    use super::super::queries::name_of;
    use super::*;

    // London (51.5074,-0.1278) and a nearby point (52.0,0.0) plus far-off Paris.
    // `cw-london-nodate` mentions an in-box Feature but lacks dateModified.
    const FIXTURE: &str = r#"
<http://sws.geonames.org/london> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.geonames.org/ontology#Feature> .
<http://sws.geonames.org/london> <http://www.geonames.org/ontology#name> "London" .
<http://sws.geonames.org/london> <http://www.w3.org/2003/01/geo/wgs84_pos#lat> "51.5074"^^<http://www.w3.org/2001/XMLSchema#double> .
<http://sws.geonames.org/london> <http://www.w3.org/2003/01/geo/wgs84_pos#long> "-0.1278"^^<http://www.w3.org/2001/XMLSchema#double> .

<http://sws.geonames.org/nearby> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.geonames.org/ontology#Feature> .
<http://sws.geonames.org/nearby> <http://www.geonames.org/ontology#name> "Nearby" .
<http://sws.geonames.org/nearby> <http://www.w3.org/2003/01/geo/wgs84_pos#lat> "52.0"^^<http://www.w3.org/2001/XMLSchema#double> .
<http://sws.geonames.org/nearby> <http://www.w3.org/2003/01/geo/wgs84_pos#long> "0.0"^^<http://www.w3.org/2001/XMLSchema#double> .

<http://sws.geonames.org/paris> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.geonames.org/ontology#Feature> .
<http://sws.geonames.org/paris> <http://www.geonames.org/ontology#name> "Paris" .
<http://sws.geonames.org/paris> <http://www.w3.org/2003/01/geo/wgs84_pos#lat> "48.8566"^^<http://www.w3.org/2001/XMLSchema#double> .
<http://sws.geonames.org/paris> <http://www.w3.org/2003/01/geo/wgs84_pos#long> "2.3522"^^<http://www.w3.org/2001/XMLSchema#double> .

<http://ex/cw-london> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw-london> <http://www.bbc.co.uk/ontologies/creativework/title> "London" .
<http://ex/cw-london> <http://www.bbc.co.uk/ontologies/creativework/dateModified> "2024-03-05T09:30:00.000Z"^^<http://www.w3.org/2001/XMLSchema#dateTime> .
<http://ex/cw-london> <http://www.bbc.co.uk/ontologies/creativework/mentions> <http://sws.geonames.org/london> .

<http://ex/cw-nearby> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw-nearby> <http://www.bbc.co.uk/ontologies/creativework/title> "Nearby" .
<http://ex/cw-nearby> <http://www.bbc.co.uk/ontologies/creativework/dateModified> "2024-02-20T12:00:00.000Z"^^<http://www.w3.org/2001/XMLSchema#dateTime> .
<http://ex/cw-nearby> <http://www.bbc.co.uk/ontologies/creativework/mentions> <http://sws.geonames.org/nearby> .

<http://ex/cw-paris> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw-paris> <http://www.bbc.co.uk/ontologies/creativework/title> "Paris" .
<http://ex/cw-paris> <http://www.bbc.co.uk/ontologies/creativework/dateModified> "2024-01-10T08:00:00.000Z"^^<http://www.w3.org/2001/XMLSchema#dateTime> .
<http://ex/cw-paris> <http://www.bbc.co.uk/ontologies/creativework/mentions> <http://sws.geonames.org/paris> .

<http://ex/cw-london-nodate> <http://www.w3.org/1999/02/22-rdf-syntax-ns#type> <http://www.bbc.co.uk/ontologies/creativework/CreativeWork> .
<http://ex/cw-london-nodate> <http://www.bbc.co.uk/ontologies/creativework/title> "NoDate" .
<http://ex/cw-london-nodate> <http://www.bbc.co.uk/ontologies/creativework/mentions> <http://sws.geonames.org/london> .
"#;

    fn titles(g: &GraphSnapshot, works: &[u32]) -> Vec<String> {
        let mut t: Vec<String> = works.iter().map(|&w| name_of(g, w).to_string()).collect();
        t.sort();
        t
    }

    #[test]
    fn returns_works_mentioning_in_box_features() {
        let g = load_str(FIXTURE).0;
        // +/-1 deg around London covers London and Nearby; Paris is far outside,
        // and the dateModified-less work is rejected by the required triple.
        assert_eq!(
            titles(&g, &run(&g, 51.5074, -0.1278, 1.0)),
            ["London", "Nearby"]
        );
    }

    #[test]
    fn tighter_deviation_narrows_the_box() {
        let g = load_str(FIXTURE).0;
        // The drill-down's shrinking box: +/-0.3 deg drops Nearby (lat 52.0).
        assert_eq!(titles(&g, &run(&g, 51.5074, -0.1278, 0.3)), ["London"]);
    }

    #[test]
    fn empty_when_nothing_in_range() {
        let g = load_str(FIXTURE).0;
        assert!(run(&g, 0.0, 0.0, 0.1).is_empty());
    }
}

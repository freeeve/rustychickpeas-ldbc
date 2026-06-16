//! Validate the SPB family on **real** generated SPB data: load the
//! self-contained N-Triples extract and run the real-vocabulary full-text and
//! geo queries (fts over `cwork:description`; geo via `cwork:mentions` ->
//! geonames `Feature` with wgs84 lat/long). Prints results and timings.
//!
//! Usage: spb_real <file.nt> [fts-word] [lat] [lon] [km]

use std::collections::HashSet;
use std::path::Path;
use std::time::Instant;

use rustychickpeas_core::{Direction, GraphSnapshot};
use rustychickpeas_ldbc::harness::time_query;
use rustychickpeas_ldbc::spb::loader;

/// Creative works mentioning a geonames Feature within `km` of `(lat, lon)`.
fn geo_works(g: &GraphSnapshot, lat: f64, lon: f64, km: f64) -> Vec<u32> {
    let mut works: HashSet<u32> = HashSet::new();
    for f in g.geo_within_radius("Feature", "lat", "long", lat, lon, km).iter() {
        for w in g.neighbors_by_type(f, Direction::Incoming, "mentions") {
            if g.nodes_with_label("CreativeWork").is_some_and(|ns| ns.contains(w)) {
                works.insert(w);
            }
        }
    }
    works.into_iter().collect()
}

fn main() -> rustychickpeas_ldbc::Result<()> {
    let path = std::env::args().nth(1).expect("usage: spb_real <file.nt> [word] [lat] [lon] [km]");
    let word = std::env::args().nth(2).unwrap_or_else(|| "federation".to_string());
    let lat: f64 = std::env::args().nth(3).and_then(|s| s.parse().ok()).unwrap_or(51.5074);
    let lon: f64 = std::env::args().nth(4).and_then(|s| s.parse().ok()).unwrap_or(-0.1278);
    let km: f64 = std::env::args().nth(5).and_then(|s| s.parse().ok()).unwrap_or(50.0);

    let t = Instant::now();
    let (g, s) = loader::load_ntriples(Path::new(&path))?;
    println!(
        "Loaded {} resources, {} triples ({} edges, {} props) in {:.2}s",
        s.resources, s.triples, s.edges, s.literals, t.elapsed().as_secs_f64()
    );
    println!(
        "  CreativeWorks: {}   geonames Features: {}",
        g.nodes_with_label("CreativeWork").map(|n| n.len()).unwrap_or(0),
        g.nodes_with_label("Feature").map(|n| n.len()).unwrap_or(0),
    );

    println!("\n-- full-text (cwork:description) --");
    for w in [word.as_str(), "election", "tennis", "federation"] {
        println!("  fts '{}': {} works", w, g.fts("CreativeWork", "description", w).len());
    }

    println!("\n-- geo (cwork:mentions -> Feature near point) --");
    let near = g.geo_within_radius("Feature", "lat", "long", lat, lon, km);
    let works = geo_works(&g, lat, lon, km);
    println!("  Features within {km}km of ({lat},{lon}): {}", near.len());
    println!("  Works mentioning a place there: {}", works.len());

    println!("\n-- geo ∩ fts --");
    let geo_set: HashSet<u32> = works.into_iter().collect();
    let hits = g.fts("CreativeWork", "description", &word).iter().filter(|w| geo_set.contains(w)).count();
    println!("  near point AND description '{}': {} works", word, hits);

    println!("\nTimings (median of 5):");
    time_query("fts description", 5, || g.fts("CreativeWork", "description", &word).len());
    time_query("geo radius (Feature)", 5, || g.geo_within_radius("Feature", "lat", "long", lat, lon, km).len());
    time_query("geo -> works", 5, || geo_works(&g, lat, lon, km).len());
    Ok(())
}

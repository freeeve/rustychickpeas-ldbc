//! SPB family — load RDF (N-Triples) into the property graph and run hand-coded
//! SPB-style queries, with **no SPARQL engine**. The aggregation/hierarchy
//! queries are plain traversals; the full-text and geo queries use the core
//! `fts` and `geo_within_radius` capabilities. See `docs/families.md`,
//! `docs/core-features.md`, and `tasks/010`.

pub mod loader;
pub mod ntriples;
pub mod queries;

use std::path::PathBuf;
use std::time::Instant;

use rustychickpeas_core::GraphSnapshot;

use crate::harness::{emit_json, jstr, time_query, Result};
use crate::props::pstr;

const DEFAULT_SAMPLE: &str = "samples/spb-sample.nt";

/// Render nodes as a JSON array of their `uri` properties (the cross-engine key).
fn uri_list(g: &GraphSnapshot, nodes: impl IntoIterator<Item = u32>) -> String {
    let items: Vec<String> = nodes
        .into_iter()
        .filter_map(|n| pstr(g, n, "uri"))
        .map(jstr)
        .collect();
    format!("[{}]", items.join(","))
}

/// Load an N-Triples file (arg 1, default `samples/spb-sample.nt`), then print
/// and time the SPB-style queries.
pub fn run() -> Result<()> {
    let path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_SAMPLE));

    eprintln!("Loading RDF (N-Triples) from {} ...", path.display());
    let t = Instant::now();
    let (g, stats) = loader::load_ntriples(&path)?;
    let secs = t.elapsed().as_secs_f64();

    println!("\n=== LDBC SPB (RDF -> property graph, no SPARQL) ===");
    println!(
        "Loaded {} resources from {} triples ({} edges, {} literal props) in {:.3}s",
        stats.resources, stats.triples, stats.edges, stats.literals, secs
    );

    let about = queries::works_about_entity(&g, "London");
    println!("\nWorks about 'London': {}", about.len());
    for &w in about.iter().take(5) {
        println!("   {}", queries::name_of(&g, w));
    }

    println!("Category rollup under 'Sport':");
    for (cat, n) in queries::works_by_category_rollup(&g, "Sport") {
        println!("   {cat:<12} {n} works");
    }

    println!(
        "\nFull-text 'football': {} works",
        queries::fts_works(&g, "football").len()
    );
    println!(
        "Near London (<=50km): {} works",
        queries::geo_works_near(&g, 51.5074, -0.1278, 50.0).len()
    );
    println!(
        "Near London AND 'tennis': {} works",
        queries::geo_fts_works(&g, 51.5074, -0.1278, 50.0, "tennis").len()
    );

    // Emit query results (as uris) plus the geo pre-filter, for the Kùzu
    // cross-check and the hybrid (b) mode. See kuzu/run_spb.py.
    let near_places = g.geo_within_radius("Place", "lat", "long", 51.5074, -0.1278, 50.0);
    let body = format!(
        "{{\n  \"about_london\": {},\n  \"fts_football\": {},\n  \"geo_near_london\": {},\n  \"geo_places_near_london\": {},\n  \"geo_fts_tennis\": {}\n}}",
        uri_list(&g, about.iter().copied()),
        uri_list(&g, queries::fts_works(&g, "football")),
        uri_list(&g, queries::geo_works_near(&g, 51.5074, -0.1278, 50.0)),
        uri_list(&g, near_places.iter()),
        uri_list(&g, queries::geo_fts_works(&g, 51.5074, -0.1278, 50.0, "tennis")),
    );
    emit_json("results", "spb.rust.json", body);
    println!("\nWrote results/spb.rust.json (uris) for the Kùzu cross-check.");

    println!("\nTimings (median of 5):");
    time_query("SPB about-entity", 5, || {
        queries::works_about_entity(&g, "London").len()
    });
    time_query("SPB category rollup", 5, || {
        queries::works_by_category_rollup(&g, "Sport").len()
    });
    time_query("SPB full-text", 5, || queries::fts_works(&g, "football").len());
    time_query("SPB geo radius", 5, || {
        queries::geo_works_near(&g, 51.5074, -0.1278, 50.0).len()
    });
    time_query("SPB geo + fts", 5, || {
        queries::geo_fts_works(&g, 51.5074, -0.1278, 50.0, "tennis").len()
    });

    Ok(())
}

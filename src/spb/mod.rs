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

use crate::harness::{time_query, Result};

const DEFAULT_SAMPLE: &str = "samples/spb-sample.nt";

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

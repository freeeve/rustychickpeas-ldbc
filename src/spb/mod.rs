//! SPB family — load RDF (N-Triples / N-Quads; the 4th n-quads term is ignored)
//! into the property graph and run the hand-coded SPB queries, with **no SPARQL
//! engine**. Full-text (q8) and geo (q6) use the core `full_text_search` / `geo` indexes; the
//! remaining aggregation queries are tasks 015–048. See `docs/families.md`,
//! `tasks/014`, and the `spb-real-data-pipeline` memory.

pub mod loader;
pub mod ntriples;
pub mod parity;
pub mod queries;

// Per-query modules (one SPB query each), filled by tasks 015–048. Pre-declared
// so parallel work never collides on a shared file.
pub mod a1; // advanced q1 (works about/mentioning a thing)
pub mod a10; // advanced q10 (works with the most mentions)
pub mod a13; // advanced q13 (works + tags + categories; subPropertyOf tag)
pub mod a14; // advanced q14 (full star; subPropertyOf tag)
pub mod a15; // advanced q15 (about & mentions share an entity type; subClassOf)
pub mod a16; // advanced q16 (works + tags, title ~ policy; subPropertyOf tag)
pub mod a17; // advanced q17 (geo)
pub mod a18; // advanced q18 (date-range)
pub mod a19; // advanced q19 (popular tags, aggregation)
pub mod a2; // advanced q2 (one work + its subtype; subClassOf)
pub mod a20; // advanced q20 (full-text)
pub mod a21; // advanced q21 (faceted search)
pub mod a22; // advanced q22 (faceted search)
pub mod a23; // advanced q23 (faceted search)
pub mod a24; // advanced q24 (relatedness timeline)
pub mod a25;
pub mod a3; // advanced q3 (works per minute in a window)
pub mod a4; // advanced q4 (work counts per subtype; subClassOf)
pub mod a5; // advanced q5 (popular topics by category; subClassOf entity type)
pub mod a6; // advanced q6 (popular about-types by coverage/audience)
pub mod a7; // advanced q7 (popular mentions gated by primaryContent count)
pub mod a8; // advanced q8 (popular topics by type/audience/date; subPropertyOf tag)
pub mod a9; // advanced q9 (max mentions on a work)
pub mod q1;
pub mod q2;
pub mod q3;
pub mod q4;
pub mod q5;
pub mod q7;
pub mod q9; // basic q9 (related works by shared-tag score) // advanced q25 (related entities)

use std::path::PathBuf;
use std::time::Instant;

use rustychickpeas_core::GraphSnapshot;

use crate::harness::{emit_json, jstr, time_query, Result};
use crate::props::pstr;

const DEFAULT_SAMPLE: &str = "samples/spb-sample.nt";
const DEF_LAT: f64 = 51.5074; // London
const DEF_LON: f64 = -0.1278;
const DEF_KM: f64 = 50.0;
const DEF_WORD: &str = "football";

/// Render nodes as a JSON array of their `uri` properties (the cross-engine key).
fn uri_list(g: &GraphSnapshot, nodes: impl IntoIterator<Item = u32>) -> String {
    let items: Vec<String> = nodes
        .into_iter()
        .filter_map(|n| pstr(g, n, "uri"))
        .map(jstr)
        .collect();
    format!("[{}]", items.join(","))
}

/// Load an N-Triples/N-Quads file (arg 1, default `samples/spb-sample.nt`) and
/// run the real-vocabulary SPB queries.
///
/// Usage: `spb [file] [word] [lat] [lon] [km]`
pub fn run() -> Result<()> {
    let a: Vec<String> = std::env::args().collect();
    let path = a
        .get(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_SAMPLE));
    let word = a.get(2).map(String::as_str).unwrap_or(DEF_WORD).to_string();
    let lat = a.get(3).and_then(|s| s.parse().ok()).unwrap_or(DEF_LAT);
    let lon = a.get(4).and_then(|s| s.parse().ok()).unwrap_or(DEF_LON);
    let km = a.get(5).and_then(|s| s.parse().ok()).unwrap_or(DEF_KM);

    eprintln!("Loading RDF from {} ...", path.display());
    let t = Instant::now();
    let (g, stats) = loader::load_ntriples(&path)?;
    let secs = t.elapsed().as_secs_f64();

    println!("\n=== LDBC SPB (RDF -> property graph, no SPARQL) ===");
    println!(
        "Loaded {} resources from {} triples ({} edges, {} literal props) in {:.3}s",
        stats.resources, stats.triples, stats.edges, stats.literals, secs
    );
    println!(
        "  CreativeWorks: {}   geonames Features: {}",
        g.nodes_with_label("CreativeWork")
            .map(|n| n.len())
            .unwrap_or(0),
        g.nodes_with_label("Feature").map(|n| n.len()).unwrap_or(0)
    );

    let q8 = queries::q8_fulltext(&g, &word);
    let q6 = queries::q6_geo(&g, lat, lon, km);
    let q68 = queries::q6_q8(&g, lat, lon, km, &word);
    println!("\nq8 full-text '{word}': {} works", q8.len());
    for &w in q8.iter().take(3) {
        println!("   {}", queries::name_of(&g, w));
    }
    println!("q6 geo (<= {km}km of {lat},{lon}): {} works", q6.len());
    println!("q6 AND q8: {} works", q68.len());

    // Emit uris for the Oxigraph cross-check (scripts/spb_crosscheck.py).
    let body = format!(
        "{{\n  \"q8_fulltext\": {},\n  \"q6_geo\": {},\n  \"q6_q8\": {},\n  \"word\": {}, \"lat\": {lat}, \"lon\": {lon}, \"km\": {km}\n}}",
        uri_list(&g, q8.iter().copied()),
        uri_list(&g, q6.iter().copied()),
        uri_list(&g, q68.iter().copied()),
        jstr(&word),
    );
    emit_json("results", "spb.rust.json", body);
    println!("\nWrote results/spb.rust.json for the Oxigraph cross-check.");

    println!("\nTimings (median of 5):");
    time_query("q8 full-text", 5, || queries::q8_fulltext(&g, &word).len());
    time_query("q6 geo", 5, || queries::q6_geo(&g, lat, lon, km).len());
    time_query("q6 AND q8", 5, || {
        queries::q6_q8(&g, lat, lon, km, &word).len()
    });

    Ok(())
}

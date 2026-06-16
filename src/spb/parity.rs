//! SPB parity runner — execute every feasible SPB query on a loaded extract with
//! one fixed, data-derived parameter set, and emit each query's **full** result
//! set as JSON (`results/spb.parity.rust.json`) for the Oxigraph cross-check
//! (`scripts/spb_parity.py`). See `tasks/052`.
//!
//! LIMITs are disabled (queries run with `usize::MAX`) so the comparison is over
//! complete result sets, sidestepping engine-specific tie-breaks at a LIMIT
//! boundary; per-query ordering is covered by the module unit tests. Result
//! identity is the work / entity `uri`; aggregates emit `(key, count)` rows.

use std::path::PathBuf;
use std::time::Instant;

use rustychickpeas_core::GraphSnapshot;

use super::{a17, a18, a19, a20, a21, a22, a23, a24, a25, loader, q1, q2, q3, q4, q5, q7};
use crate::harness::{emit_json, jstr, Result};
use crate::props::pstr;

const DEFAULT_EXTRACT: &str = "data/spb/extract/spb-validate.nt";

// Fixed, data-derived parameters — each chosen (via SPARQL discovery on the
// extract) to yield non-empty results. Kept identical to the SPARQL side by
// emitting them into the params block the harness reads.
const WORD: &str = "football";
const TOPIC: &str = "http://dbpedia.org/resource/Action_of_25_February_1781";
// ASCII co-occurrence partner of TOPIC (14 shared works). A high-overlap partner
// exists (Ottoman–Portuguese_conflicts, 3759) but its IRI is non-ASCII and the
// extract encodes it inconsistently (works.nt percent-encodes `%E2%80%93`,
// entities.nt keeps raw UTF-8 `–`), which the loader does not canonicalize — see
// tasks/052. An ASCII partner keeps a24 a real test rather than an artifact.
const ENT_B: &str = "http://dbpedia.org/resource/International_Telecoms_Week";
const CATEGORY: &str = "http://www.bbc.co.uk/category/Event";
const AUDIENCE: &str = "http://www.bbc.co.uk/ontologies/creativework/InternationalAudience";
const CW_TYPE: &str = "BlogPost";
const DATE_FROM: &str = "2011-03-01T00:00:00.000+00:00";
const DATE_TO: &str = "2011-06-01T00:00:00.000+00:00";
const LAT: f64 = 51.5074;
const LON: f64 = -0.1278;
const DEVIATION: f64 = 0.5;
const ALL: usize = usize::MAX;

/// JSON-quoted `uri` of a node (the cross-engine identity key).
fn uri(g: &GraphSnapshot, n: u32) -> String {
    jstr(pstr(g, n, "uri").unwrap_or("?"))
}

/// A JSON array of node uris (result order is not significant — see module docs).
fn uris(g: &GraphSnapshot, ns: &[u32]) -> String {
    let items: Vec<String> = ns.iter().map(|&n| uri(g, n)).collect();
    format!("[{}]", items.join(","))
}

/// One query block: `{"kind":<kind>,"rows":<rows>}`.
fn block(kind: &str, rows: String) -> String {
    format!("{{\"kind\":{},\"rows\":{}}}", jstr(kind), rows)
}

/// `[[label, count], ...]` for `(String, usize)` aggregate rows.
fn rows_kv(items: &[(String, usize)]) -> String {
    let v: Vec<String> = items.iter().map(|(k, n)| format!("[{},{}]", jstr(k), n)).collect();
    format!("[{}]", v.join(","))
}

/// Load the extract (arg 1, default the SPB-10 validation extract) and emit every
/// query's full result set plus the parameter block, then print rust-side timings.
///
/// Usage: `spb_parity [extract.nt]`
pub fn run() -> Result<()> {
    let a: Vec<String> = std::env::args().collect();
    let path = a.get(1).map(PathBuf::from).unwrap_or_else(|| PathBuf::from(DEFAULT_EXTRACT));

    eprintln!("Loading {} ...", path.display());
    let t = Instant::now();
    let (g, stats) = loader::load_ntriples(&path)?;
    eprintln!(
        "Loaded {} resources / {} triples in {:.2}s",
        stats.resources,
        stats.triples,
        t.elapsed().as_secs_f64()
    );

    // q2 needs a concrete creative work; use the newest work tagging the topic
    // (q1's first row) so the SPARQL side can pin the same one.
    let q1_works = q1::run(&g, TOPIC);
    let q2_cw = q1_works.first().map(|&w| pstr(&g, w, "uri").unwrap_or("").to_string());
    let q2_in = q2_cw.as_deref().unwrap_or("");

    // a19 rows are (key-label, count, extra-label) — serialize all three.
    let a19_rows = {
        let r = a19::run(&g, Some(CW_TYPE), Some(AUDIENCE), DATE_FROM, DATE_TO, ALL);
        let v: Vec<String> =
            r.iter().map(|(k, n, x)| format!("[{},{},{}]", jstr(k), n, jstr(x))).collect();
        format!("[{}]", v.join(","))
    };
    // a24 rows are ((year, month, day), count) -> "YYYY-MM-DD" key.
    let a24_rows = {
        let r = a24::run(&g, TOPIC, ENT_B, None, None);
        let v: Vec<String> = r
            .iter()
            .map(|((y, m, d), n)| format!("[{},{}]", jstr(&format!("{y:04}-{m:02}-{d:02}")), n))
            .collect();
        format!("[{}]", v.join(","))
    };
    // a25 rows are (who-node, interactionDays) -> (who-uri, days).
    let a25_rows = {
        let r = a25::run(&g, TOPIC, ALL);
        let v: Vec<String> = r.iter().map(|(w, n)| format!("[{},{}]", uri(&g, *w), n)).collect();
        format!("[{}]", v.join(","))
    };

    let queries = format!(
        concat!(
            "{{",
            "\"q1\":{},\"q2\":{},\"q3\":{},\"q4\":{},\"q5\":{},\"q7\":{},",
            "\"a17\":{},\"a18\":{},\"a19\":{},\"a20\":{},\"a21\":{},\"a22\":{},",
            "\"a23\":{},\"a24\":{},\"a25\":{}",
            "}}"
        ),
        block("uris", uris(&g, &q1_works)),
        block("uri_opt", uris(&g, &q2::run(&g, q2_in).into_iter().collect::<Vec<_>>())),
        block("uris", uris(&g, &q3::run(&g, TOPIC, ALL))),
        block("uris", uris(&g, &q4::run(&g, TOPIC, ALL))),
        block("kv", rows_kv(&q5::run(&g, Some(CW_TYPE), Some(AUDIENCE), DATE_FROM, DATE_TO))),
        block("uris", uris(&g, &q7::run(&g, CW_TYPE, DATE_FROM, DATE_TO, Some(CATEGORY), Some(AUDIENCE)))),
        block("uris", uris(&g, &a17::run(&g, LAT, LON, DEVIATION))),
        block("uris", uris(&g, &a18::run(&g, CW_TYPE, DATE_FROM, DATE_TO, ALL))),
        block("kvx", a19_rows),
        block("uris", uris(&g, &a20::run(&g, WORD, ALL))),
        block(
            "uris",
            uris(&g, &a21::run(&g, WORD, Some(CATEGORY), Some(AUDIENCE), None, None, None, None, ALL)),
        ),
        block(
            "uris",
            uris(&g, &a22::run(&g, WORD, Some(CATEGORY), Some(AUDIENCE), None, Some(DATE_FROM), Some(DATE_TO), None, ALL)),
        ),
        block("kv", rows_kv(&a23::run(&g, WORD, CATEGORY, ALL))),
        block("day_count", a24_rows),
        block("who_days", a25_rows),
    );

    let params = format!(
        concat!(
            "{{\"word\":{},\"topic\":{},\"entB\":{},\"category\":{},\"audience\":{},",
            "\"cwType\":{},\"dateFrom\":{},\"dateTo\":{},\"lat\":{},\"lon\":{},",
            "\"deviation\":{},\"q2_cw\":{}}}"
        ),
        jstr(WORD), jstr(TOPIC), jstr(ENT_B), jstr(CATEGORY), jstr(AUDIENCE),
        jstr(CW_TYPE), jstr(DATE_FROM), jstr(DATE_TO), LAT, LON, DEVIATION, jstr(q2_in),
    );

    emit_json("results", "spb.parity.rust.json", format!("{{\"params\":{params},\"queries\":{queries}}}"));
    println!("Wrote results/spb.parity.rust.json");

    // Rust-side timings (median of 3), plus a row-count summary.
    println!("\n{:<6}{:>10}  {:>8}", "query", "rust ms", "rows");
    time("q1", 3, &mut || q1::run(&g, TOPIC).len());
    time("q3", 3, &mut || q3::run(&g, TOPIC, ALL).len());
    time("q4", 3, &mut || q4::run(&g, TOPIC, ALL).len());
    time("q5", 3, &mut || q5::run(&g, Some(CW_TYPE), Some(AUDIENCE), DATE_FROM, DATE_TO).len());
    time("q7", 3, &mut || q7::run(&g, CW_TYPE, DATE_FROM, DATE_TO, Some(CATEGORY), Some(AUDIENCE)).len());
    time("a17", 3, &mut || a17::run(&g, LAT, LON, DEVIATION).len());
    time("a18", 3, &mut || a18::run(&g, CW_TYPE, DATE_FROM, DATE_TO, ALL).len());
    time("a20", 3, &mut || a20::run(&g, WORD, ALL).len());
    time("a24", 3, &mut || a24::run(&g, TOPIC, ENT_B, None, None).len());
    time("a25", 3, &mut || a25::run(&g, TOPIC, ALL).len());

    Ok(())
}

/// Time `q` over `runs` repetitions and print `name  median-ms  last-row-count`.
fn time(name: &str, runs: usize, q: &mut impl FnMut() -> usize) {
    let mut ms: Vec<f64> = Vec::with_capacity(runs);
    let mut rows = 0;
    for _ in 0..runs {
        let t = Instant::now();
        rows = q();
        ms.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    ms.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!("{:<6}{:>10.3}  {:>8}", name, ms[runs / 2], rows);
}

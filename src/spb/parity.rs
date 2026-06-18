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

use super::{
    a1, a10, a13, a14, a15, a16, a17, a18, a19, a2, a20, a21, a22, a23, a24, a25, a3, a4, a5, a6,
    a7, a8, a9, loader, q1, q2, q3, q4, q5, q7, q9,
};
use crate::harness::{emit_json, jstr, Result};
use crate::props::PropExt;

const DEFAULT_EXTRACT: &str = "data/spb/extract/spb-validate.nt";

// Fixed, data-derived parameters — each chosen (via SPARQL discovery on the
// extract) to yield non-empty results. Kept identical to the SPARQL side by
// emitting them into the params block the harness reads.
const WORD: &str = "football";
const WORD2: &str = "policy"; // rarer title word for the star queries (q15/q16)
const TOPIC: &str = "http://dbpedia.org/resource/Action_of_25_February_1781";
// ASCII co-occurrence partner of TOPIC (14 shared works). A high-overlap partner
// exists (Ottoman–Portuguese_conflicts, 3759) but its IRI is non-ASCII and the
// extract encodes it inconsistently (works.nt percent-encodes `%E2%80%93`,
// entities.nt keeps raw UTF-8 `–`), which the loader does not canonicalize — see
// tasks/052. An ASCII partner keeps a24 a real test rather than an artifact.
const ENT_B: &str = "http://dbpedia.org/resource/International_Telecoms_Week";
const CATEGORY: &str = "http://www.bbc.co.uk/category/Event";
const CAT_COMPANY: &str = "http://www.bbc.co.uk/category/Company";
// coreconcepts:Thing, the super-class dbo:Company/Event carry via subClassOf;
// our loader materializes it as the label `Thing` (q5's entity-type restriction).
const ENTITY_LABEL: &str = "Thing";
const PRIMARY_FORMAT: &str = "http://www.bbc.co.uk/ontologies/creativework/TextualFormat";
const WEB_DOC_TYPE: &str = "http://www.bbc.co.uk/ontologies/bbc/HighWeb";
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
    jstr(g.prop(n, "uri").str().unwrap_or("?"))
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
    let v: Vec<String> = items
        .iter()
        .map(|(k, n)| format!("[{},{}]", jstr(k), n))
        .collect();
    format!("[{}]", v.join(","))
}

/// A JSON array of strings (e.g. type local names).
fn strs(items: &[String]) -> String {
    let v: Vec<String> = items.iter().map(|s| jstr(s)).collect();
    format!("[{}]", v.join(","))
}

/// `[[a, b], ...]` for `(String, String)` pair rows (e.g. (work, tag) uris).
fn rows_pairs(items: &[(String, String)]) -> String {
    let v: Vec<String> = items
        .iter()
        .map(|(a, b)| format!("[{},{}]", jstr(a), jstr(b)))
        .collect();
    format!("[{}]", v.join(","))
}

/// Load the extract (arg 1, default the SPB-10 validation extract) and emit every
/// query's full result set plus the parameter block, then print rust-side timings.
///
/// Usage: `spb_parity [extract.nt]`
pub fn run() -> Result<()> {
    let a: Vec<String> = std::env::args().collect();
    let path = a
        .get(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_EXTRACT));

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
    let q2_cw = q1_works
        .first()
        .map(|&w| g.prop(w, "uri").str().unwrap_or("").to_string());
    let q2_in = q2_cw.as_deref().unwrap_or("");

    // a19 rows are (key-label, count, extra-label) — serialize all three.
    let a19_rows = {
        let r = a19::run(&g, Some(CW_TYPE), Some(AUDIENCE), DATE_FROM, DATE_TO, ALL);
        let v: Vec<String> = r
            .iter()
            .map(|(k, n, x)| format!("[{},{},{}]", jstr(k), n, jstr(x)))
            .collect();
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
        let v: Vec<String> = r
            .iter()
            .map(|(w, n)| format!("[{},{}]", uri(&g, *w), n))
            .collect();
        format!("[{}]", v.join(","))
    };

    let queries = format!(
        concat!(
            "{{",
            "\"q1\":{},\"q2\":{},\"q3\":{},\"q4\":{},\"q5\":{},\"q7\":{},",
            "\"a17\":{},\"a18\":{},\"a19\":{},\"a20\":{},\"a21\":{},\"a22\":{},",
            "\"a23\":{},\"a24\":{},\"a25\":{},\"a5\":{},\"a8\":{},",
            "\"a1\":{},\"a2\":{},\"a3\":{},\"a4\":{},\"a6\":{},\"a7\":{},",
            "\"a9\":{},\"a10\":{},\"a13\":{},\"a14\":{},\"a16\":{},\"a15\":{},\"q9\":{}",
            "}}"
        ),
        block("uris", uris(&g, &q1_works)),
        block(
            "uri_opt",
            uris(&g, &q2::run(&g, q2_in).into_iter().collect::<Vec<_>>())
        ),
        block("uris", uris(&g, &q3::run(&g, TOPIC, ALL))),
        block("uris", uris(&g, &q4::run(&g, TOPIC, ALL))),
        block(
            "kv",
            rows_kv(&q5::run(
                &g,
                Some(CW_TYPE),
                Some(AUDIENCE),
                DATE_FROM,
                DATE_TO
            ))
        ),
        block(
            "uris",
            uris(
                &g,
                &q7::run(
                    &g,
                    CW_TYPE,
                    DATE_FROM,
                    DATE_TO,
                    Some(CATEGORY),
                    Some(AUDIENCE)
                )
            )
        ),
        block("uris", uris(&g, &a17::run(&g, LAT, LON, DEVIATION))),
        block(
            "uris",
            uris(&g, &a18::run(&g, CW_TYPE, DATE_FROM, DATE_TO, ALL))
        ),
        block("kvx", a19_rows),
        block("uris", uris(&g, &a20::run(&g, WORD, ALL))),
        block(
            "uris",
            uris(
                &g,
                &a21::run(
                    &g,
                    WORD,
                    Some(CATEGORY),
                    Some(AUDIENCE),
                    None,
                    None,
                    None,
                    None,
                    ALL
                )
            ),
        ),
        block(
            "uris",
            uris(
                &g,
                &a22::run(
                    &g,
                    WORD,
                    Some(CATEGORY),
                    Some(AUDIENCE),
                    None,
                    Some(DATE_FROM),
                    Some(DATE_TO),
                    None,
                    ALL
                )
            ),
        ),
        block("kv", rows_kv(&a23::run(&g, WORD, CATEGORY, ALL))),
        block("day_count", a24_rows),
        block("who_days", a25_rows),
        block(
            "kv",
            rows_kv(&a5::run(&g, ENTITY_LABEL, CAT_COMPANY, CATEGORY, ALL))
        ),
        block(
            "kv",
            rows_kv(&a8::run(&g, CW_TYPE, AUDIENCE, DATE_FROM, DATE_TO))
        ),
        block("uris", uris(&g, &a1::run(&g, "about", TOPIC))),
        block("uris", strs(&a2::run(&g, q2_in))),
        block("kv", rows_kv(&a3::run(&g, DATE_FROM, DATE_TO))),
        block("kv", rows_kv(&a4::run(&g, DATE_FROM, DATE_TO, ALL))),
        block("kv", rows_kv(&a6::run(&g, true, AUDIENCE, ALL))),
        block("kv", rows_kv(&a7::run(&g, 1, ALL))),
        block("kv", format!("[[{},{}]]", jstr("max"), a9::run(&g))),
        block("kv", rows_kv(&a10::run(&g, ALL))),
        block(
            "pairs",
            rows_pairs(&a13::run(&g, CAT_COMPANY, CATEGORY, ALL))
        ),
        block(
            "uris",
            uris(&g, &a14::run(&g, PRIMARY_FORMAT, WEB_DOC_TYPE, ALL))
        ),
        block("pairs", rows_pairs(&a16::run(&g, WORD2, ALL))),
        block("uris", uris(&g, &a15::run(&g, WORD2, ALL))),
        // q9 score emitted x2 as an integer (weights 4/3/2/1) to match the SPARQL,
        // which avoids Oxigraph's decimal-coefficient arithmetic quirk.
        block(
            "kv",
            rows_kv(
                &q9::run(&g, q2_in, ALL)
                    .into_iter()
                    .map(|(u, s)| (u, (s * 2.0).round() as usize))
                    .collect::<Vec<_>>(),
            ),
        ),
    );

    let params = format!(
        concat!(
            "{{\"word\":{},\"topic\":{},\"entB\":{},\"category\":{},\"audience\":{},",
            "\"cwType\":{},\"dateFrom\":{},\"dateTo\":{},\"lat\":{},\"lon\":{},",
            "\"deviation\":{},\"q2_cw\":{},\"catCompany\":{},\"entityType\":{},",
            "\"word2\":{},\"live\":\"true\",\"threshold\":1,\"maxMentions\":{},",
            "\"primaryFormat\":{},\"webDocType\":{}}}"
        ),
        jstr(WORD),
        jstr(TOPIC),
        jstr(ENT_B),
        jstr(CATEGORY),
        jstr(AUDIENCE),
        jstr(CW_TYPE),
        jstr(DATE_FROM),
        jstr(DATE_TO),
        LAT,
        LON,
        DEVIATION,
        jstr(q2_in),
        jstr(CAT_COMPANY),
        jstr("http://www.bbc.co.uk/ontologies/coreconcepts/Thing"),
        jstr(WORD2),
        a9::run(&g),
        jstr(PRIMARY_FORMAT),
        jstr(WEB_DOC_TYPE),
    );

    emit_json(
        "results",
        "spb.parity.rust.json",
        format!("{{\"params\":{params},\"queries\":{queries}}}"),
    );
    println!("Wrote results/spb.parity.rust.json");

    // Rust-side timings (median of 5) over the full result set, all 30 queries,
    // with per-query allocation count and bytes (the bin installs a counting
    // allocator; see src/alloc_count.rs).
    let n = 5;
    println!(
        "\n{:<6}{:>10}{:>10}{:>11}{:>9}",
        "query", "rust ms", "allocs", "bytes", "rows"
    );
    time("q1", n, &mut || q1::run(&g, TOPIC).len());
    time("q2", n, &mut || q2::run(&g, q2_in).into_iter().count());
    time("q3", n, &mut || q3::run(&g, TOPIC, ALL).len());
    time("q4", n, &mut || q4::run(&g, TOPIC, ALL).len());
    time("q5", n, &mut || {
        q5::run(&g, Some(CW_TYPE), Some(AUDIENCE), DATE_FROM, DATE_TO).len()
    });
    time("q7", n, &mut || {
        q7::run(
            &g,
            CW_TYPE,
            DATE_FROM,
            DATE_TO,
            Some(CATEGORY),
            Some(AUDIENCE),
        )
        .len()
    });
    time("q9", n, &mut || q9::run(&g, q2_in, ALL).len());
    time("a1", n, &mut || a1::run(&g, "about", TOPIC).len());
    time("a2", n, &mut || a2::run(&g, q2_in).len());
    time("a3", n, &mut || a3::run(&g, DATE_FROM, DATE_TO).len());
    time("a4", n, &mut || a4::run(&g, DATE_FROM, DATE_TO, ALL).len());
    time("a5", n, &mut || {
        a5::run(&g, ENTITY_LABEL, CAT_COMPANY, CATEGORY, ALL).len()
    });
    time("a6", n, &mut || a6::run(&g, true, AUDIENCE, ALL).len());
    time("a7", n, &mut || a7::run(&g, 1, ALL).len());
    time("a8", n, &mut || {
        a8::run(&g, CW_TYPE, AUDIENCE, DATE_FROM, DATE_TO).len()
    });
    time("a9", n, &mut || a9::run(&g));
    time("a10", n, &mut || a10::run(&g, ALL).len());
    time("a13", n, &mut || {
        a13::run(&g, CAT_COMPANY, CATEGORY, ALL).len()
    });
    time("a14", n, &mut || {
        a14::run(&g, PRIMARY_FORMAT, WEB_DOC_TYPE, ALL).len()
    });
    time("a15", n, &mut || a15::run(&g, WORD2, ALL).len());
    time("a16", n, &mut || a16::run(&g, WORD2, ALL).len());
    time("a17", n, &mut || a17::run(&g, LAT, LON, DEVIATION).len());
    time("a18", n, &mut || {
        a18::run(&g, CW_TYPE, DATE_FROM, DATE_TO, ALL).len()
    });
    time("a19", n, &mut || {
        a19::run(&g, Some(CW_TYPE), Some(AUDIENCE), DATE_FROM, DATE_TO, ALL).len()
    });
    time("a20", n, &mut || a20::run(&g, WORD, ALL).len());
    time("a21", n, &mut || {
        a21::run(
            &g,
            WORD,
            Some(CATEGORY),
            Some(AUDIENCE),
            None,
            None,
            None,
            None,
            ALL,
        )
        .len()
    });
    time("a22", n, &mut || {
        a22::run(
            &g,
            WORD,
            Some(CATEGORY),
            Some(AUDIENCE),
            None,
            Some(DATE_FROM),
            Some(DATE_TO),
            None,
            ALL,
        )
        .len()
    });
    time("a23", n, &mut || a23::run(&g, WORD, CATEGORY, ALL).len());
    time("a24", n, &mut || {
        a24::run(&g, TOPIC, ENT_B, None, None).len()
    });
    time("a25", n, &mut || a25::run(&g, TOPIC, ALL).len());

    Ok(())
}

/// Time `q` over `runs` repetitions (median) and measure its allocations on one
/// isolated run, printing `name  median-ms  allocs  bytes  rows`.
fn time(name: &str, runs: usize, q: &mut impl FnMut() -> usize) {
    // One bracketed run for the (deterministic) allocation tally.
    crate::alloc_count::reset();
    let rows = q();
    let (allocs, bytes) = crate::alloc_count::read();

    let mut ms: Vec<f64> = Vec::with_capacity(runs);
    for _ in 0..runs {
        let t = Instant::now();
        q();
        ms.push(t.elapsed().as_secs_f64() * 1000.0);
    }
    ms.sort_by(|a, b| a.partial_cmp(b).unwrap());
    println!(
        "{:<6}{:>10.3}{:>10}{:>11}{:>9}",
        name,
        ms[runs / 2],
        allocs,
        bytes,
        rows
    );
}

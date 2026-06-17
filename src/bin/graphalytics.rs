//! LDBC Graphalytics runner: load a `.v`/`.e`/`.properties` dataset, run the six
//! algorithms with the dataset's parameters, validate each against any present
//! `<name>-<ALGO>` reference output, and time it.
//!
//! Usage: `graphalytics [dataset-dir] [dataset-name]`
//! (defaults: `data/graphalytics` / `example-directed`).

use std::collections::HashMap;
use std::path::Path;
use std::time::{Duration, Instant};

use rustychickpeas_ldbc::graphalytics as ga;
use rustychickpeas_ldbc::Result;

/// Load `<dir>/<name>-<ALGO>` as a reference map, or `None` when absent.
fn reference(dir: &Path, name: &str, algo: &str) -> Option<HashMap<u32, String>> {
    std::fs::read_to_string(dir.join(format!("{name}-{algo}")))
        .ok()
        .map(|t| ga::validate::parse_reference(&t))
}

/// Print one algorithm's timing, result size, and validation verdict.
fn show(algo: &str, dur: Duration, n: usize, check: Option<std::result::Result<(), String>>) {
    let status = match check {
        None => "(no reference)".to_string(),
        Some(Ok(())) => "PASS".to_string(),
        Some(Err(e)) => format!("FAIL: {e}"),
    };
    println!("  {algo:<5}{:>9.2} ms   n={n:<8} {status}", dur.as_secs_f64() * 1000.0);
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let dir = Path::new(args.get(1).map(String::as_str).unwrap_or("data/graphalytics"));
    let name = args.get(2).map(String::as_str).unwrap_or("example-directed");

    let t = Instant::now();
    let ds = ga::load(dir, name)?;
    let g = &ds.graph;
    let d = ds.params.directed;
    println!("Loaded {name}: {} vertices, directed={d}  [{:.2}s]", ds.len(), t.elapsed().as_secs_f64());
    println!("Graphalytics algorithms:");

    // BFS — exact depths.
    let src = ds.params.bfs_source.and_then(|v| ds.node(v)).unwrap_or(0);
    let t = Instant::now();
    let r = ga::bfs(g, src, d);
    let dur = t.elapsed();
    let chk = reference(dir, name, "BFS").map(|rf| ga::validate::check_exact_i64(&ds, &r, &rf));
    show("BFS", dur, r.len(), chk);

    // PageRank — tolerance.
    let t = Instant::now();
    let r = ga::pagerank(g, d, ds.params.pr_damping, ds.params.pr_iterations);
    let dur = t.elapsed();
    let chk = reference(dir, name, "PR").map(|rf| ga::validate::check_epsilon(&ds, &r, &rf, 1e-6));
    show("PR", dur, r.len(), chk);

    // WCC — relabel-invariant.
    let t = Instant::now();
    let r = ga::wcc(g);
    let dur = t.elapsed();
    let chk = reference(dir, name, "WCC").map(|rf| ga::validate::check_relabel(&ds, &r, &rf));
    show("WCC", dur, r.len(), chk);

    // CDLP — exact labels; seed with vertex ids so labels match the reference.
    let t = Instant::now();
    let r = ga::cdlp_seeded(g, d, ds.params.cdlp_iterations, &ds.vertex_of_node);
    let dur = t.elapsed();
    let r_i64: Vec<i64> = r.iter().map(|&x| x as i64).collect();
    let chk = reference(dir, name, "CDLP").map(|rf| ga::validate::check_exact_i64(&ds, &r_i64, &rf));
    show("CDLP", dur, r.len(), chk);

    // LCC — tolerance.
    let t = Instant::now();
    let r = ga::lcc(g, d);
    let dur = t.elapsed();
    let chk = reference(dir, name, "LCC").map(|rf| ga::validate::check_epsilon(&ds, &r, &rf, 1e-6));
    show("LCC", dur, r.len(), chk);

    // SSSP — tolerance (weighted).
    let src = ds.params.sssp_source.and_then(|v| ds.node(v)).unwrap_or(0);
    let t = Instant::now();
    let r = ga::sssp(g, src, d, true);
    let dur = t.elapsed();
    let chk = reference(dir, name, "SSSP").map(|rf| ga::validate::check_epsilon(&ds, &r, &rf, 1e-6));
    show("SSSP", dur, r.len(), chk);

    Ok(())
}

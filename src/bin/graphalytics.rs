//! LDBC Graphalytics runner: load a `.v`/`.e`/`.properties` dataset, run the six
//! algorithms with the dataset's parameters, and report per-algorithm wall-clock
//! time, allocation count/bytes, and validation against any present
//! `<name>-<ALGO>` reference output.
//!
//! Usage: `graphalytics [dataset-dir] [dataset-name]`
//! (defaults: `data/graphalytics` / `example-directed`).

use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

use rustychickpeas_ldbc::graphalytics as ga;
use rustychickpeas_ldbc::{alloc_count, Result};

/// Count allocations so each algorithm can report its own allocs/bytes.
#[global_allocator]
static GLOBAL: alloc_count::CountingAlloc = alloc_count::CountingAlloc;

/// Load `<dir>/<name>-<ALGO>` as a reference map, or `None` when absent.
fn reference(dir: &Path, name: &str, algo: &str) -> Option<HashMap<u32, String>> {
    std::fs::read_to_string(dir.join(format!("{name}-{algo}")))
        .ok()
        .map(|t| ga::validate::parse_reference(&t))
}

/// Run `f` under the allocation counter and a wall-clock timer, returning its
/// result and `(ms, allocs, bytes)`. The reset/read brackets only `f`, so the
/// caller's later validation work is excluded from the tallies.
fn measure<T>(f: impl FnOnce() -> T) -> (T, f64, u64, u64) {
    alloc_count::reset();
    let t = Instant::now();
    let r = f();
    let ms = t.elapsed().as_secs_f64() * 1000.0;
    let (allocs, bytes) = alloc_count::read();
    (r, ms, allocs, bytes)
}

/// Print one algorithm's time, allocations, result size, and validation verdict.
fn row(algo: &str, ms: f64, allocs: u64, bytes: u64, n: usize, check: Option<std::result::Result<(), String>>) {
    let status = match check {
        None => "(no ref)".to_string(),
        Some(Ok(())) => "PASS".to_string(),
        Some(Err(e)) => format!("FAIL: {e}"),
    };
    println!("  {algo:<5}{ms:>9.2} ms  allocs={allocs:>9}  bytes={bytes:>12}  n={n:<8} {status}");
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
    let (r, ms, a, b) = measure(|| ga::bfs(g, src, d));
    let chk = reference(dir, name, "BFS").map(|rf| ga::validate::check_exact_i64(&ds, &r, &rf));
    row("BFS", ms, a, b, r.len(), chk);

    // PageRank — tolerance.
    let (r, ms, a, b) = measure(|| ga::pagerank(g, d, ds.params.pr_damping, ds.params.pr_iterations));
    let chk = reference(dir, name, "PR").map(|rf| ga::validate::check_epsilon(&ds, &r, &rf, 1e-6));
    row("PR", ms, a, b, r.len(), chk);

    // WCC — relabel-invariant.
    let (r, ms, a, b) = measure(|| ga::wcc(g));
    let chk = reference(dir, name, "WCC").map(|rf| ga::validate::check_relabel(&ds, &r, &rf));
    row("WCC", ms, a, b, r.len(), chk);

    // CDLP — exact labels; seed with vertex ids so labels match the reference.
    let (r, ms, a, b) = measure(|| ga::cdlp_seeded(g, d, ds.params.cdlp_iterations, &ds.vertex_of_node));
    let r_i64: Vec<i64> = r.iter().map(|&x| x as i64).collect();
    let chk = reference(dir, name, "CDLP").map(|rf| ga::validate::check_exact_i64(&ds, &r_i64, &rf));
    row("CDLP", ms, a, b, r.len(), chk);

    // LCC — tolerance.
    let (r, ms, a, b) = measure(|| ga::lcc(g, d));
    let chk = reference(dir, name, "LCC").map(|rf| ga::validate::check_epsilon(&ds, &r, &rf, 1e-6));
    row("LCC", ms, a, b, r.len(), chk);

    // SSSP — tolerance (weighted).
    let src = ds.params.sssp_source.and_then(|v| ds.node(v)).unwrap_or(0);
    let (r, ms, a, b) = measure(|| ga::sssp(g, src, d, true));
    let chk = reference(dir, name, "SSSP").map(|rf| ga::validate::check_epsilon(&ds, &r, &rf, 1e-6));
    row("SSSP", ms, a, b, r.len(), chk);

    Ok(())
}

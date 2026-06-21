//! Output (JSON dump) and timing harness shared by all query families.

use std::error::Error;
use std::sync::OnceLock;
use std::time::Instant;

pub type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// Per-run benchmark configuration set once from the bin's CLI flags.
#[derive(Clone, Default)]
pub struct BenchCfg {
    /// Run only the query whose first name token equals this (lowercased), e.g. `"ic5"`.
    pub only: Option<String>,
    /// Override the timed-iteration count; `0` keeps each call site's default.
    pub runs: usize,
    /// Report allocation count/bytes for one call instead of wall-clock time.
    pub alloc: bool,
}
static CFG: OnceLock<BenchCfg> = OnceLock::new();

/// Install the benchmark configuration (idempotent; first call wins).
pub fn set_bench_cfg(cfg: BenchCfg) {
    let _ = CFG.set(cfg);
}
fn cfg() -> BenchCfg {
    CFG.get().cloned().unwrap_or_default()
}
pub fn jstr(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Write cross-check JSON (an array of row-arrays) to `<dir>/<name>`.
pub fn emit_json(dir: &str, name: &str, body: String) {
    let _ = std::fs::create_dir_all(dir);
    if let Err(e) = std::fs::write(format!("{dir}/{name}"), body) {
        eprintln!("emit_json {name}: {e}");
    }
}
/// Median wall-clock over `runs` timed iterations (after one warmup).
///
/// Honors the global [`BenchCfg`]: `--only` skips non-matching queries,
/// `--repeat` overrides `runs`, and `--alloc` reports allocation count/bytes
/// (deterministic, load-independent) for one call instead of timing.
pub fn time_query(name: &str, runs: usize, mut q: impl FnMut() -> usize) {
    let c = cfg();
    if let Some(only) = &c.only {
        let id = name.split_whitespace().next().unwrap_or("").to_lowercase();
        if &id != only {
            return;
        }
    }
    let warm = q();

    if c.alloc {
        crate::alloc_count::reset();
        let r = q();
        let (allocs, bytes) = crate::alloc_count::read();
        println!("{name:<34} allocs={allocs:>10}  bytes={bytes:>13}  (result={r})");
        return;
    }

    let runs = if c.runs > 0 { c.runs } else { runs };
    let mut samples: Vec<u128> = Vec::with_capacity(runs);
    for _ in 0..runs {
        let t = Instant::now();
        let _ = q();
        samples.push(t.elapsed().as_micros());
    }
    samples.sort_unstable();
    let median_ms = samples[samples.len() / 2] as f64 / 1000.0;
    println!("{name:<34} {median_ms:>9.2} ms   (result={warm})");
}

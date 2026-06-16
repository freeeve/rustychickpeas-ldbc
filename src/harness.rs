//! Output (JSON dump) and timing harness shared by all query families.

use std::error::Error;
use std::time::Instant;

pub type Result<T> = std::result::Result<T, Box<dyn Error>>;
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
pub fn time_query(name: &str, runs: usize, mut q: impl FnMut() -> usize) {
    let warm = q();
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


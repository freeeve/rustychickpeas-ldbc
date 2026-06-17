//! FinBench loader: load a generated `raw/` directory, print node/edge counts
//! and load time, and verify an edge's timestamp + amount are readable during
//! traversal (the relationship-accessor capability the queries rely on).
//!
//!   cargo run --release --bin finbench -- data/finbench/raw

use std::path::Path;

use rustychickpeas_core::Direction;
use rustychickpeas_ldbc::finbench;

fn main() {
    let dir = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "data/finbench/raw".to_string());

    let (g, s) = match finbench::load_finbench(Path::new(&dir)) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("load error: {e}");
            std::process::exit(1);
        }
    };

    println!(
        "FinBench {dir}: {} nodes, {} edges in {} ms (snapshot {} nodes / {} rels)",
        s.nodes,
        s.edges,
        s.load_ms,
        g.node_count(),
        g.relationship_count()
    );

    // 007 acceptance: edge timestamp + amount readable during traversal.
    for n in 0..g.node_count() {
        if let Some(r) = g.relationships(n, Direction::Outgoing, "transfer").next() {
            println!(
                "  sample transfer {n} -> {}: ts={:?} amt={:?}",
                r.neighbor,
                g.relationship_property(r.pos, "ts"),
                g.relationship_property(r.pos, "amt")
                    .and_then(|v| v.to_f64()),
            );
            break;
        }
    }
}

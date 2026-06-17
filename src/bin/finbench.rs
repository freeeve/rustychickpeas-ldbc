//! FinBench loader: load a generated `raw/` directory, print node/edge counts
//! and load time, and verify an edge's timestamp + amount are readable during
//! traversal (the relationship-accessor capability the queries rely on).
//!
//!   cargo run --release --bin finbench -- data/finbench/raw

use std::path::Path;

use rustychickpeas_core::{Direction, GraphSnapshot};
use rustychickpeas_ldbc::{finbench, harness};

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

    run_queries(&g);
}

/// Pick representative seeds (highest transfer degree / guarantee degree) and run
/// + time the four transaction-tracing queries (timing-only — no published
/// comparison implied).
fn run_queries(g: &GraphSnapshot) {
    let win = 90 * 86_400_000i64;

    // Accounts sorted by transfer degree (descending) — the seed pool.
    let mut by_deg: Vec<(u32, usize)> = g
        .nodes_with_label("Account")
        .map(|s| {
            s.iter()
                .map(|a| (a, g.relationships(a, Direction::Both, "transfer").count()))
                .collect()
        })
        .unwrap_or_default();
    by_deg.sort_by_key(|&(_, d)| std::cmp::Reverse(d));
    let seed = by_deg.first().map(|&(a, _)| a).unwrap_or(0);

    // Cycle seed: the first high-degree account actually on a time-ordered
    // transfer cycle (FinBench cycles are sparse — a random hub isn't on one).
    let (cyc_seed, cyc) = by_deg
        .iter()
        .take(500)
        .map(|&(a, _)| (a, finbench::transfer_cycles(g, a, 1000.0, win).len()))
        .find(|&(_, c)| c > 0)
        .unwrap_or((seed, 0));

    // Reachable destination: up to 4 outgoing transfer hops from the seed.
    let mut dst = seed;
    for _ in 0..4 {
        match g
            .relationships(dst, Direction::Outgoing, "transfer")
            .map(|r| r.neighbor)
            .find(|&n| n != seed)
        {
            Some(n) => dst = n,
            None => break,
        }
    }

    // Person seed: highest outgoing-guarantee degree.
    let seed_person = g
        .nodes_with_label("Person")
        .and_then(|s| {
            s.iter()
                .max_by_key(|&p| g.relationships(p, Direction::Outgoing, "guarantee").count())
        })
        .unwrap_or(0);

    println!(
        "seeds: account={seed} (deg {}), cycle-account={cyc_seed}, dst={dst}, person={seed_person}",
        by_deg.first().map(|&(_, d)| d).unwrap_or(0)
    );
    println!(
        "  trace_transfers_in(<=3 hops): {} upstream accounts",
        finbench::trace_transfers_in(g, seed, i64::MIN, i64::MAX, 3).len()
    );
    println!("  transfer_cycles(>=1000, 90d): {cyc} cycles");
    println!(
        "  shortest_transfer_path({seed}->{dst}): {} hops",
        finbench::shortest_transfer_path(g, seed, dst, i64::MIN, i64::MAX)
    );
    println!(
        "  guarantee_exposure(person {seed_person}): {:.2}",
        finbench::guarantee_exposure(g, seed_person)
    );

    let runs = 30;
    harness::time_query("FB1 trace_transfers_in", runs, || {
        finbench::trace_transfers_in(g, seed, i64::MIN, i64::MAX, 3).len()
    });
    harness::time_query("FB2 transfer_cycles", runs, || {
        finbench::transfer_cycles(g, cyc_seed, 1000.0, win).len()
    });
    harness::time_query("FB3 shortest_transfer_path", runs, || {
        finbench::shortest_transfer_path(g, seed, dst, i64::MIN, i64::MAX).max(0) as usize
    });
    harness::time_query("FB4 guarantee_exposure", runs, || {
        finbench::guarantee_exposure(g, seed_person) as usize
    });
}

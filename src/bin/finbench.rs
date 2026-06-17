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

    // Seeds for the loan/medium/company CRs: max outgoing-degree by relationship.
    let max_by = |label: &str, rel: &str| -> u32 {
        g.nodes_with_label(label)
            .and_then(|s| {
                s.iter()
                    .max_by_key(|&n| g.relationships(n, Direction::Outgoing, rel).count())
            })
            .unwrap_or(0)
    };
    let card = max_by("Account", "withdraw");
    let loan_seed = max_by("Loan", "deposit");
    let investor = max_by("Person", "invest");
    let owner = max_by("Person", "own");
    // CR10 needs two persons sharing an invested company.
    let coinvestor = g
        .relationships(investor, Direction::Outgoing, "invest")
        .flat_map(|r| {
            g.relationships(r.neighbor, Direction::Incoming, "invest")
                .map(|x| x.neighbor)
        })
        .find(|&p| p != investor)
        .unwrap_or(investor);
    println!(
        "seeds: account={seed} person={seed_person} owner={owner} card={card} loan={loan_seed} investor={investor}/{coinvestor} cycle={cyc_seed} dst={dst}"
    );
    let to = finbench::TruncationOrder::Descending;
    let (ws, we) = (i64::MIN, i64::MAX);
    // Result shapes (full window) — confirm non-empty before timing.
    println!(
        "  CR1:{} CR2:{} CR3:{}h CR4:{}cyc CR5:{} CR6:{} CR7:{:?} CR8:{} CR9:{:?} CR10:{:.0} CR11:{:.0} CR12:{}",
        finbench::cr1(g, seed, ws, we, 10_000, false).len(),
        finbench::cr2(g, owner, ws, we, 10_000, false).len(),
        finbench::shortest_transfer_path(g, seed, dst, ws, we),
        cyc,
        finbench::cr5(g, owner, ws, we, 10_000, "desc").len(),
        finbench::cr6(g, card, 0.0, 0.0, ws, we, 10_000, "desc").len(),
        finbench::cr7(g, seed, 0.0, ws, we, 10_000, to),
        finbench::cr8(g, loan_seed, 0.0, ws, we, 10_000, "desc").len(),
        finbench::cr9(g, seed, 0.0, ws, we, 10_000, false),
        finbench::cr10(g, investor, coinvestor, ws, we),
        finbench::guarantee_exposure(g, seed_person),
        finbench::cr12(g, owner, ws, we, 10_000, to).len(),
    );

    let runs = 30;
    harness::time_query("CR1 blocked-medium", runs, || {
        finbench::cr1(g, seed, ws, we, 10_000, false).len()
    });
    harness::time_query("CR2 loan-gather", runs, || {
        finbench::cr2(g, owner, ws, we, 10_000, false).len()
    });
    harness::time_query("CR3 shortest-path", runs, || {
        finbench::shortest_transfer_path(g, seed, dst, ws, we).max(0) as usize
    });
    harness::time_query("CR4 3-cycle", runs, || {
        finbench::transfer_cycles(g, cyc_seed, 1000.0, win).len()
    });
    harness::time_query("CR5 downstream-trace", runs, || {
        finbench::cr5(g, owner, ws, we, 10_000, "desc").len()
    });
    harness::time_query("CR6 withdraw-after-in", runs, || {
        finbench::cr6(g, card, 0.0, 0.0, ws, we, 10_000, "desc").len()
    });
    harness::time_query("CR7 in-out-ratio", runs, || {
        finbench::cr7(g, seed, 0.0, ws, we, 10_000, to).0 as usize
    });
    harness::time_query("CR8 loan-fund-trace", runs, || {
        finbench::cr8(g, loan_seed, 0.0, ws, we, 10_000, "desc").len()
    });
    harness::time_query("CR9 laundering", runs, || {
        finbench::cr9(g, seed, 0.0, ws, we, 10_000, false).0 as usize
    });
    harness::time_query("CR10 investor-sim", runs, || {
        finbench::cr10(g, investor, coinvestor, ws, we) as usize
    });
    harness::time_query("CR11 guarantee-chain", runs, || {
        finbench::guarantee_exposure(g, seed_person) as usize
    });
    harness::time_query("CR12 company-transfer", runs, || {
        finbench::cr12(g, owner, ws, we, 10_000, to).len()
    });
}

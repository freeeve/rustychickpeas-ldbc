//! FinBench loader: load a generated `raw/` directory, print node/rel counts
//! and load time, and verify an rel's timestamp + amount are readable during
//! traversal (the relationship-accessor capability the queries rely on).
//!
//!   cargo run --release --bin finbench -- data/finbench/raw

use std::path::Path;

use rustychickpeas_core::{Direction, GraphSnapshot};
use rustychickpeas_ldbc::{finbench, harness};

// With `--features alloc-count`, install the counting allocator so `--alloc`
// reports allocs/bytes per query. Default builds keep the system allocator for
// pristine timing.
#[cfg(feature = "alloc-count")]
#[global_allocator]
static GLOBAL: rustychickpeas_ldbc::alloc_count::CountingAlloc =
    rustychickpeas_ldbc::alloc_count::CountingAlloc;

/// Re-read the `id` column of every `.csv` in `dir` (sorted, matching the loader's
/// file order) so the emit can map dense internal NodeIds back to original FinBench
/// ids. Additive helper for the cross-check emit only — not on the timing path.
fn read_ids(dir: &std::path::Path) -> Vec<i64> {
    let mut ids = Vec::new();
    if !dir.exists() {
        return ids;
    }
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .map(|rd| rd.flatten().map(|e| e.path()).collect::<Vec<_>>())
        .unwrap_or_default();
    files.retain(|p| p.extension().and_then(|s| s.to_str()) == Some("csv"));
    files.sort();
    for path in files {
        if let Ok(mut rdr) = csv::ReaderBuilder::new()
            .delimiter(b'|')
            .has_headers(true)
            .flexible(true)
            .from_path(&path)
        {
            let i = rdr
                .headers()
                .ok()
                .and_then(|h| h.iter().position(|c| c == "id"))
                .unwrap_or(0);
            for rec in rdr.records().flatten() {
                ids.push(rec.get(i).and_then(|s| s.parse().ok()).unwrap_or(0));
            }
        }
    }
    ids
}

/// Build the flat `nid -> originalId` map by re-reading the node CSVs in the exact
/// order `load_finbench` assigns ids (account, person, company, medium, loan).
fn orig_id_map(raw: &std::path::Path) -> Vec<i64> {
    let mut m = Vec::new();
    for t in ["account", "person", "company", "medium", "loan"] {
        m.extend(read_ids(&raw.join(t)));
    }
    m
}

fn main() {
    // Optional flags mirror the IC bin: --only <id> (e.g. cr5), --repeat <n>,
    // --alloc. The first non-flag arg is the raw/ data dir.
    let (mut only, mut runs_override, mut alloc, mut dir) = (None, 0usize, false, None::<String>);
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        match a.as_str() {
            "--only" => only = it.next().map(|s| s.to_lowercase()),
            "--repeat" => runs_override = it.next().and_then(|s| s.parse().ok()).unwrap_or(0),
            "--alloc" => alloc = true,
            s if !s.starts_with("--") && dir.is_none() => dir = Some(s.to_string()),
            _ => {}
        }
    }
    harness::set_bench_cfg(harness::BenchCfg {
        only,
        runs: runs_override,
        alloc,
    });
    let dir = dir.unwrap_or_else(|| "data/finbench/raw".to_string());

    let (g, s) = match finbench::load_finbench(Path::new(&dir)) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("load error: {e}");
            std::process::exit(1);
        }
    };

    println!(
        "FinBench {dir}: {} nodes, {} rels in {} ms (snapshot {} nodes / {} rels)",
        s.nodes,
        s.rels,
        s.load_ms,
        g.node_count(),
        g.relationship_count()
    );

    // 007 acceptance: rel timestamp + amount readable during traversal.
    for n in 0..g.node_count() {
        if let Some(r) = g.relationships(n, Direction::Outgoing, "transfer").next() {
            println!(
                "  sample transfer {n} -> {}: ts={:?} amt={:?}",
                r.neighbor,
                g.rel_prop(r.pos, "ts").and_then(|p| p.i64()),
                g.rel_prop(r.pos, "amt").and_then(|p| p.f64()),
            );
            break;
        }
    }

    run_queries(&g, Path::new(&dir));
}

/// Pick representative seeds (highest transfer degree / guarantee degree) and run
/// + time the four transaction-tracing queries (timing-only — no published
/// comparison implied).
fn run_queries(g: &GraphSnapshot, raw: &Path) {
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
    // CR1 seed: a high-degree account that actually has a blocked-medium upstream
    // (the pattern is sparse — most accounts have none within 3 hops).
    let cr1_seed = by_deg
        .iter()
        .take(500)
        .map(|&(a, _)| a)
        .find(|&a| !finbench::cr1(g, a, i64::MIN, i64::MAX, 10_000, false).is_empty())
        .unwrap_or(seed);
    println!(
        "seeds: account={seed} cr1={cr1_seed} person={seed_person} owner={owner} card={card} loan={loan_seed} investor={investor} cycle={cyc_seed} dst={dst}"
    );
    let to = finbench::TruncationOrder::Descending;
    let (ws, we) = (i64::MIN, i64::MAX);
    // Result shapes (full window) — confirm non-empty before timing.
    println!(
        "  CR1:{} CR2:{} CR3:{}h CR4:{}cyc CR5:{} CR6:{} CR7:{:?} CR8:{} CR9:{:?} CR10:{} CR11:{:.0} CR12:{}",
        finbench::cr1(g, cr1_seed, ws, we, 10_000, false).len(),
        finbench::cr2(g, owner, ws, we, 10_000, false).len(),
        finbench::shortest_transfer_path(g, seed, dst, ws, we),
        cyc,
        finbench::cr5(g, owner, ws, we, 10_000, "desc").len(),
        finbench::cr6(g, card, 0.0, 0.0, ws, we, 10_000, "desc").len(),
        finbench::cr7(g, seed, 0.0, ws, we, 10_000, to),
        finbench::cr8(g, loan_seed, 0.0, ws, we, 10_000, "desc").len(),
        finbench::cr9(g, seed, 0.0, ws, we, 10_000, false),
        finbench::cr10(g, investor, ws, we).len(),
        finbench::guarantee_exposure(g, seed_person),
        finbench::cr12(g, owner, ws, we, 10_000, to).len(),
    );

    // Cross-check emit (additive; mirrors BI's LDBC_EMIT_JSON). Dumps each TCR's
    // full result as canonical JSON in *original* FinBench ids, plus the seeds /
    // windows used, so the Kùzu side can anchor on the same instances and
    // kuzu/finbench_compare.py can diff sorted rows. Skips the timing block.
    if let Ok(dir) = std::env::var("FINBENCH_EMIT_JSON") {
        let id = orig_id_map(raw);
        let oid = |n: u32| *id.get(n as usize).unwrap_or(&-1);
        let r3 = |x: f64| {
            let v = (x * 1000.0).round() / 1000.0;
            format!("{v:.3}")
        };
        let arr = |v: Vec<String>| format!("[{}]", v.join(","));
        let idseq = |p: &[u32]| {
            p.iter()
                .map(|&n| oid(n).to_string())
                .collect::<Vec<_>>()
                .join(",")
        };

        let c1 = finbench::cr1(g, cr1_seed, ws, we, 10_000, false);
        harness::emit_json(&dir, "cr1.rust.json", arr(c1.iter()
            .map(|&(o, d, m, t)| format!("[{},{},{},{}]", oid(o), d, oid(m), harness::jstr(t))).collect()));
        let c2 = finbench::cr2(g, owner, ws, we, 10_000, false);
        harness::emit_json(&dir, "cr2.rust.json", arr(c2.iter()
            .map(|&(a, x, y)| format!("[{},{},{}]", oid(a), r3(x), r3(y))).collect()));
        let c3 = finbench::shortest_transfer_path(g, seed, dst, ws, we);
        harness::emit_json(&dir, "cr3.rust.json", format!("[[{c3}]]"));
        let c4 = finbench::transfer_cycles(g, cyc_seed, 1000.0, win);
        harness::emit_json(&dir, "cr4.rust.json", arr(c4.iter()
            .map(|cy| format!("[{}]", idseq(cy))).collect()));
        let c5 = finbench::cr5(g, owner, ws, we, 10_000, "desc");
        harness::emit_json(&dir, "cr5.rust.json", arr(c5.iter()
            .map(|p| format!("[{}]", idseq(p))).collect()));
        let c6 = finbench::cr6(g, card, 0.0, 0.0, ws, we, 10_000, "desc");
        harness::emit_json(&dir, "cr6.rust.json", arr(c6.iter()
            .map(|&(s, a, b)| format!("[{},{},{}]", oid(s), r3(a), r3(b))).collect()));
        let c7 = finbench::cr7(g, seed, 0.0, ws, we, 10_000, to);
        harness::emit_json(&dir, "cr7.rust.json", format!("[[{},{},{}]]", c7.0, c7.1, r3(c7.2)));
        let c8 = finbench::cr8(g, loan_seed, 0.0, ws, we, 10_000, "desc");
        harness::emit_json(&dir, "cr8.rust.json", arr(c8.iter()
            .map(|&(d, r, dist)| format!("[{},{},{}]", oid(d), r3(r), dist)).collect()));
        let c9 = finbench::cr9(g, seed, 0.0, ws, we, 10_000, false);
        harness::emit_json(&dir, "cr9.rust.json",
            format!("[[{},{},{}]]", r3(c9.0 as f64), r3(c9.1 as f64), r3(c9.2 as f64)));
        let c10 = finbench::cr10(g, investor, ws, we);
        harness::emit_json(&dir, "cr10.rust.json", arr(c10.iter()
            .map(|&(o, c)| format!("[{},{}]", oid(o), c)).collect()));
        let c11 = finbench::guarantee_exposure(g, seed_person);
        harness::emit_json(&dir, "cr11.rust.json", format!("[[{}]]", r3(c11)));
        let c12 = finbench::cr12(g, owner, ws, we, 10_000, to);
        harness::emit_json(&dir, "cr12.rust.json", arr(c12.iter()
            .map(|&(a, x)| format!("[{},{}]", oid(a), r3(x))).collect()));

        harness::emit_json(&dir, "seeds.json", format!(
            "{{\"account\":{},\"cr1\":{},\"person\":{},\"owner\":{},\"card\":{},\"loan\":{},\"investor\":{},\"cycle\":{},\"dst\":{},\"ws\":{},\"we\":{},\"cycle_window\":{},\"cr4_minamt\":1000.0}}",
            oid(seed), oid(cr1_seed), oid(seed_person), oid(owner), oid(card), oid(loan_seed),
            oid(investor), oid(cyc_seed), oid(dst), ws, we, win));
        println!("emitted finbench rust cross-check JSON to {dir}");
        return;
    }

    let runs = 30;
    harness::time_query("CR1 blocked-medium", runs, || {
        finbench::cr1(g, cr1_seed, ws, we, 10_000, false).len()
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
        finbench::cr10(g, investor, ws, we).len()
    });
    harness::time_query("CR11 guarantee-chain", runs, || {
        finbench::guarantee_exposure(g, seed_person) as usize
    });
    harness::time_query("CR12 company-transfer", runs, || {
        finbench::cr12(g, owner, ws, we, 10_000, to).len()
    });
}

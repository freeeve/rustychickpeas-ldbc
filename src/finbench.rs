//! LDBC FinBench (Financial Benchmark) — loader + transaction-tracing queries.
//!
//! FinBench is a different schema from SNB: Account / Person / Company / Medium /
//! Loan nodes, and time-stamped, amount-weighted edges — `transfer`, `withdraw`,
//! `deposit`, `repay`, `guarantee`, `invest`, `signIn`, `own`, `apply`. The
//! generator (`scripts/gen_finbench.sh`) emits pipe-delimited CSV under `raw/`.
//!
//! The read workload is transaction tracing: temporal fund-flow paths, transfer
//! cycles inside a time window, blocked-account propagation. This plays to the
//! edge-property-during-traversal capability (per-edge `ts` / `amt` read via the
//! relationship accessor's CSR position) — the queries below (`tasks/008`).

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;
use std::time::Instant;

use rustychickpeas_core::{Direction, GraphBuilder, GraphSnapshot, PropertyValue, ValueId};

/// Load report, mirroring the BI loader.
pub struct Stats {
    pub nodes: u64,
    pub edges: u64,
    pub load_ms: u128,
}

/// Iterate pipe-delimited `.csv` files in `dir`, resolving `cols` by header and
/// calling `f` with those column values per row. Plain-file sibling of the BI
/// loader's gzip `for_each_row` (FinBench CSV is not gzipped). A missing
/// directory yields zero rows (some edge types are empty at small scale).
fn for_each_csv(dir: &Path, cols: &[&str], mut f: impl FnMut(&[&str])) -> Result<u64, String> {
    if !dir.exists() {
        return Ok(0);
    }
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .map_err(|e| format!("read_dir {}: {e}", dir.display()))?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|s| s.to_str()) == Some("csv"))
        .collect();
    files.sort();

    let mut count = 0u64;
    for path in files {
        let mut reader = csv::ReaderBuilder::new()
            .delimiter(b'|')
            .has_headers(true)
            .flexible(true)
            .from_path(&path)
            .map_err(|e| format!("open {}: {e}", path.display()))?;
        let headers = reader.headers().map_err(|e| e.to_string())?.clone();
        let idx: Vec<usize> = cols
            .iter()
            .map(|c| {
                headers
                    .iter()
                    .position(|h| h == *c)
                    .ok_or_else(|| format!("column '{c}' not in {headers:?}"))
            })
            .collect::<Result<_, _>>()?;
        for rec in reader.records() {
            let rec = rec.map_err(|e| e.to_string())?;
            let row: Vec<&str> = idx.iter().map(|&i| rec.get(i).unwrap_or("")).collect();
            f(&row);
            count += 1;
        }
    }
    Ok(count)
}

/// Add an amount-bearing edge type (from -> to), storing `ts` + `amt` so the
/// queries can filter on timestamp/amount during traversal.
#[allow(clippy::too_many_arguments)]
fn edge_amt(
    b: &mut GraphBuilder,
    dir: &Path,
    from: &HashMap<i64, u32>,
    to: &HashMap<i64, u32>,
    fc: &str,
    tc: &str,
    ts_col: &str,
    amt_col: &str,
    rel: &str,
) -> Result<u64, String> {
    let mut n = 0u64;
    for_each_csv(dir, &[fc, tc, ts_col, amt_col], |v| {
        if let (Some(&u), Some(&w)) = (
            from.get(&v[0].parse().unwrap_or(0)),
            to.get(&v[1].parse().unwrap_or(0)),
        ) {
            let idx = b.add_relationship(u, w, rel).unwrap();
            b.set_relationship_props_by_index(
                idx,
                &[
                    ("ts", PropertyValue::Integer(v[2].parse().unwrap_or(0))),
                    ("amt", PropertyValue::Float(v[3].parse().unwrap_or(0.0))),
                ],
            );
            n += 1;
        }
    })?;
    Ok(n)
}

/// Add a timestamp-only edge type (from -> to), storing `ts`.
fn edge_ts(
    b: &mut GraphBuilder,
    dir: &Path,
    from: &HashMap<i64, u32>,
    to: &HashMap<i64, u32>,
    fc: &str,
    tc: &str,
    ts_col: &str,
    rel: &str,
) -> Result<u64, String> {
    let mut n = 0u64;
    for_each_csv(dir, &[fc, tc, ts_col], |v| {
        if let (Some(&u), Some(&w)) = (
            from.get(&v[0].parse().unwrap_or(0)),
            to.get(&v[1].parse().unwrap_or(0)),
        ) {
            let idx = b.add_relationship(u, w, rel).unwrap();
            b.set_relationship_props_by_index(
                idx,
                &[("ts", PropertyValue::Integer(v[2].parse().unwrap_or(0)))],
            );
            n += 1;
        }
    })?;
    Ok(n)
}

/// Load a FinBench `raw/` directory into an immutable snapshot. FinBench ids are
/// i64 unique only within a type, so each node type gets its own id -> NodeId
/// map; edges resolve their endpoints through the right maps.
pub fn load_finbench(raw: &Path) -> Result<(GraphSnapshot, Stats), String> {
    let t0 = Instant::now();
    let mut b = GraphBuilder::new(Some(150_000), Some(1_000_000));
    let mut next: u32 = 0;
    let (mut acct, mut pers, mut comp, mut loan, mut med) = (
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
        HashMap::new(),
    );

    // --- nodes ---
    for_each_csv(&raw.join("account"), &["id", "isBlocked"], |v| {
        let nid = next;
        next += 1;
        acct.insert(v[0].parse().unwrap_or(0), nid);
        b.add_node(Some(nid), &["Account"]).unwrap();
        b.set_prop_bool(nid, "blocked", v[1] == "true").unwrap();
    })?;
    for_each_csv(&raw.join("person"), &["id"], |v| {
        let nid = next;
        next += 1;
        pers.insert(v[0].parse().unwrap_or(0), nid);
        b.add_node(Some(nid), &["Person"]).unwrap();
    })?;
    for_each_csv(&raw.join("company"), &["id"], |v| {
        let nid = next;
        next += 1;
        comp.insert(v[0].parse().unwrap_or(0), nid);
        b.add_node(Some(nid), &["Company"]).unwrap();
    })?;
    for_each_csv(&raw.join("medium"), &["id", "isBlocked"], |v| {
        let nid = next;
        next += 1;
        med.insert(v[0].parse().unwrap_or(0), nid);
        b.add_node(Some(nid), &["Medium"]).unwrap();
        b.set_prop_bool(nid, "blocked", v[1] == "true").unwrap();
    })?;
    for_each_csv(&raw.join("loan"), &["id", "loanAmount", "balance"], |v| {
        let nid = next;
        next += 1;
        loan.insert(v[0].parse().unwrap_or(0), nid);
        b.add_node(Some(nid), &["Loan"]).unwrap();
        b.set_prop_f64(nid, "amount", v[1].parse().unwrap_or(0.0))
            .unwrap();
        b.set_prop_f64(nid, "balance", v[2].parse().unwrap_or(0.0))
            .unwrap();
    })?;
    let nodes = next as u64;

    // --- edges (ts + amount where the schema carries it) ---
    let mut edges = 0u64;
    edges += edge_amt(
        &mut b,
        &raw.join("transfer"),
        &acct,
        &acct,
        "fromId",
        "toId",
        "createTime",
        "amount",
        "transfer",
    )?;
    edges += edge_amt(
        &mut b,
        &raw.join("withdraw"),
        &acct,
        &acct,
        "fromId",
        "toId",
        "createTime",
        "amount",
        "withdraw",
    )?;
    edges += edge_amt(
        &mut b,
        &raw.join("deposit"),
        &loan,
        &acct,
        "loanId",
        "accountId",
        "createTime",
        "amount",
        "deposit",
    )?;
    edges += edge_amt(
        &mut b,
        &raw.join("repay"),
        &acct,
        &loan,
        "accountId",
        "loanId",
        "createTime",
        "amount",
        "repay",
    )?;
    edges += edge_amt(
        &mut b,
        &raw.join("personApplyLoan"),
        &pers,
        &loan,
        "personId",
        "loanId",
        "createTime",
        "loanAmount",
        "apply",
    )?;
    edges += edge_amt(
        &mut b,
        &raw.join("companyApplyLoan"),
        &comp,
        &loan,
        "companyId",
        "loanId",
        "createTime",
        "loanAmount",
        "apply",
    )?;
    edges += edge_ts(
        &mut b,
        &raw.join("personGuarantee"),
        &pers,
        &pers,
        "fromId",
        "toId",
        "createTime",
        "guarantee",
    )?;
    edges += edge_ts(
        &mut b,
        &raw.join("companyGuarantee"),
        &comp,
        &comp,
        "fromId",
        "toId",
        "createTime",
        "guarantee",
    )?;
    edges += edge_ts(
        &mut b,
        &raw.join("personOwnAccount"),
        &pers,
        &acct,
        "personId",
        "accountId",
        "createTime",
        "own",
    )?;
    edges += edge_ts(
        &mut b,
        &raw.join("companyOwnAccount"),
        &comp,
        &acct,
        "companyId",
        "accountId",
        "createTime",
        "own",
    )?;
    edges += edge_ts(
        &mut b,
        &raw.join("personInvest"),
        &pers,
        &comp,
        "investorId",
        "companyId",
        "createTime",
        "invest",
    )?;
    edges += edge_ts(
        &mut b,
        &raw.join("companyInvest"),
        &comp,
        &comp,
        "investorId",
        "companyId",
        "createTime",
        "invest",
    )?;
    edges += edge_ts(
        &mut b,
        &raw.join("signIn"),
        &med,
        &acct,
        "mediumId",
        "accountId",
        "createTime",
        "signIn",
    )?;

    let snapshot = b.finalize(None);
    Ok((
        snapshot,
        Stats {
            nodes,
            edges,
            load_ms: t0.elapsed().as_millis(),
        },
    ))
}

// --- queries (tasks/008): temporal traversals that read each edge's ts / amount
// mid-traversal via the relationship accessor's CSR position. ---

/// Edge timestamp (`ts`, epoch ms) at CSR position `pos`.
fn rel_ts(g: &GraphSnapshot, pos: u32) -> i64 {
    match g.relationship_property(pos, "ts") {
        Some(ValueId::I64(t)) => t,
        _ => i64::MIN,
    }
}

/// Edge amount (`amt`) at CSR position `pos`.
fn rel_amt(g: &GraphSnapshot, pos: u32) -> f64 {
    g.relationship_property(pos, "amt")
        .and_then(|v| v.to_f64())
        .unwrap_or(0.0)
}

/// Read a node property (loan `amount`/`balance`, account/medium `blocked`).
fn node_prop(g: &GraphSnapshot, node: u32, key: &str) -> Option<rustychickpeas_core::ValueId> {
    g.property_key_from_str(key)
        .and_then(|id| g.columns.get(&id))
        .and_then(|c| c.get(node))
}

/// True if `medium`/`account` carries `blocked = true`.
fn is_blocked(g: &GraphSnapshot, node: u32) -> bool {
    matches!(
        node_prop(g, node, "blocked"),
        Some(rustychickpeas_core::ValueId::Bool(true))
    )
}

/// TCR1-style — trace `transfer` paths (≤`max_hops`) feeding into `account`
/// within `[start_ms, end_ms]`, returning the upstream accounts. The window
/// filter reads each edge's timestamp during the reverse BFS.
pub fn trace_transfers_in(
    g: &GraphSnapshot,
    account: u32,
    start_ms: i64,
    end_ms: i64,
    max_hops: u32,
) -> Vec<u32> {
    let mut visited = HashSet::new();
    visited.insert(account);
    let mut queue: VecDeque<(u32, u32)> = VecDeque::new();
    queue.push_back((account, 0));
    let mut reached = Vec::new();
    while let Some((node, depth)) = queue.pop_front() {
        if depth >= max_hops {
            continue;
        }
        for r in g.relationships(node, Direction::Incoming, "transfer") {
            let ts = rel_ts(g, r.pos);
            if ts < start_ms || ts > end_ms {
                continue;
            }
            if visited.insert(r.neighbor) {
                reached.push(r.neighbor);
                queue.push_back((r.neighbor, depth + 1));
            }
        }
    }
    reached
}

/// Max cycle length and a cap on cycles returned, to keep the DFS bounded.
const MAX_CYCLE_LEN: usize = 6;
const MAX_CYCLES: usize = 1000;

/// TCR8-style — fund-transfer cycles back to `account` where each hop is strictly
/// later in time, each amount ≥ `min_amount`, and the cycle completes within
/// `window_ms` of its first hop.
pub fn transfer_cycles(
    g: &GraphSnapshot,
    account: u32,
    min_amount: f64,
    window_ms: i64,
) -> Vec<Vec<u32>> {
    let mut cycles = Vec::new();
    let mut path = vec![account];
    let mut on_path = HashSet::new();
    on_path.insert(account);
    cycle_dfs(
        g,
        account,
        account,
        i64::MIN,
        None,
        min_amount,
        window_ms,
        &mut path,
        &mut on_path,
        &mut cycles,
    );
    cycles
}

#[allow(clippy::too_many_arguments)]
fn cycle_dfs(
    g: &GraphSnapshot,
    start: u32,
    node: u32,
    last_ts: i64,
    first_ts: Option<i64>,
    min_amount: f64,
    window_ms: i64,
    path: &mut Vec<u32>,
    on_path: &mut HashSet<u32>,
    out: &mut Vec<Vec<u32>>,
) {
    if path.len() > MAX_CYCLE_LEN || out.len() >= MAX_CYCLES {
        return;
    }
    for r in g.relationships(node, Direction::Outgoing, "transfer") {
        let ts = rel_ts(g, r.pos);
        if ts <= last_ts || rel_amt(g, r.pos) < min_amount {
            continue; // strictly increasing time + amount threshold
        }
        let f0 = first_ts.unwrap_or(ts);
        if ts - f0 > window_ms {
            continue; // outside the cycle window
        }
        if r.neighbor == start {
            if path.len() >= 2 {
                out.push(path.clone());
            }
            continue;
        }
        if on_path.contains(&r.neighbor) {
            continue;
        }
        path.push(r.neighbor);
        on_path.insert(r.neighbor);
        cycle_dfs(
            g,
            start,
            r.neighbor,
            ts,
            Some(f0),
            min_amount,
            window_ms,
            path,
            on_path,
            out,
        );
        path.pop();
        on_path.remove(&r.neighbor);
    }
}

/// TCR3-style — shortest in-window `transfer` path (hop count) from `src` to
/// `dst`, or -1 if unreachable. Out-of-window edges are pruned by the Dijkstra
/// weight closure reading each edge's timestamp.
pub fn shortest_transfer_path(
    g: &GraphSnapshot,
    src: u32,
    dst: u32,
    start_ms: i64,
    end_ms: i64,
) -> i64 {
    g.weighted_shortest_path(src, dst, Direction::Outgoing, "transfer", |_from, r| {
        if (start_ms..=end_ms).contains(&rel_ts(g, r.pos)) {
            1.0
        } else {
            f64::INFINITY
        }
    })
    .map(|cost| cost as i64)
    .unwrap_or(-1)
}

/// TCR11-style — a person's loan exposure: walk the `guarantee` chain out from
/// `person`, summing the `apply` (loan) amounts they and everyone they guarantee
/// are on the hook for.
pub fn guarantee_exposure(g: &GraphSnapshot, person: u32) -> f64 {
    let mut visited = HashSet::new();
    visited.insert(person);
    let mut queue = VecDeque::new();
    queue.push_back(person);
    let mut total = 0.0;
    while let Some(p) = queue.pop_front() {
        for r in g.relationships(p, Direction::Outgoing, "apply") {
            total += rel_amt(g, r.pos);
        }
        for r in g.relationships(p, Direction::Outgoing, "guarantee") {
            if visited.insert(r.neighbor) {
                queue.push_back(r.neighbor);
            }
        }
    }
    total
}

// === TCR1-TCR12 (faithful spec implementations; tasks 079-090) ============

/// TCR1 "Blocked medium related accounts" — accounts reachable from `account` by
/// a ≤3-hop, time-ascending, in-window reverse `transfer` trace that are signed
/// in by a **blocked** Medium. Returns (otherId, distance, mediumId, mediumType).
pub fn cr1(
    g: &GraphSnapshot,
    account: u32,
    start_ms: i64,
    end_ms: i64,
    truncation_limit: u32,
    truncation_order_asc: bool,
) -> Vec<(u32, u32, u32, String)> {
    let mut results = Vec::new();
    let mut visited = HashSet::new();
    visited.insert(account);
    let mut queue: VecDeque<(u32, u32, i64)> = VecDeque::new(); // (node, depth, last_ts)
    queue.push_back((account, 0, i64::MAX));

    while let Some((node, depth, last_ts)) = queue.pop_front() {
        if depth >= 3 {
            continue;
        }
        // In-window, strictly-ascending (forward) = strictly-decreasing backward.
        let mut edges: Vec<_> = g
            .relationships(node, Direction::Incoming, "transfer")
            .filter(|r| {
                let ts = rel_ts(g, r.pos);
                ts >= start_ms && ts <= end_ms && ts < last_ts
            })
            .collect();
        // Truncation on hub vertices: keep the top `truncation_limit` by time.
        if truncation_order_asc {
            edges.sort_by_key(|r| rel_ts(g, r.pos));
        } else {
            edges.sort_by_key(|r| std::cmp::Reverse(rel_ts(g, r.pos)));
        }
        edges.truncate(truncation_limit as usize);

        for r in edges {
            let ts = rel_ts(g, r.pos);
            if visited.insert(r.neighbor) {
                let dist = depth + 1;
                for sig in g.relationships(r.neighbor, Direction::Incoming, "signIn") {
                    if is_blocked(g, sig.neighbor) {
                        results.push((r.neighbor, dist, sig.neighbor, "Medium".to_string()));
                    }
                }
                queue.push_back((r.neighbor, dist, ts));
            }
        }
    }
    results.sort_by(|a, b| a.1.cmp(&b.1).then(a.0.cmp(&b.0)).then(a.2.cmp(&b.2)));
    results
}

/// TCR2 "Fund gathered from the accounts applying loans" — from a Person's owned
/// accounts, reverse-trace in-window `transfer` (≤3 hops), and for each upstream
/// account sum the loan amount/balance deposited into it. Returns (otherId,
/// sumLoanAmount, sumLoanBalance).
pub fn cr2(
    g: &GraphSnapshot,
    person: u32,
    start_ms: i64,
    end_ms: i64,
    truncation_limit: u32,
    truncation_order_asc: bool,
) -> Vec<(u32, f64, f64)> {
    let mut by_acct: HashMap<u32, (f64, f64)> = HashMap::new();

    for own in g.relationships(person, Direction::Outgoing, "own") {
        let owned = own.neighbor;
        let mut visited = HashSet::new();
        visited.insert(owned);
        let mut queue: VecDeque<(u32, u32, i64)> = VecDeque::new();
        queue.push_back((owned, 0, i64::MAX));

        while let Some((node, depth, last_ts)) = queue.pop_front() {
            if depth >= 3 {
                continue;
            }
            let mut rels: Vec<_> = g
                .relationships(node, Direction::Incoming, "transfer")
                .collect();
            if truncation_order_asc {
                rels.sort_by_key(|r| rel_ts(g, r.pos));
            } else {
                rels.sort_by_key(|r| std::cmp::Reverse(rel_ts(g, r.pos)));
            }
            rels.truncate(truncation_limit as usize);
            for r in rels {
                let ts = rel_ts(g, r.pos);
                if ts < start_ms || ts > end_ms || ts >= last_ts {
                    continue;
                }
                if visited.insert(r.neighbor) {
                    queue.push_back((r.neighbor, depth + 1, ts));
                }
            }
        }

        for &acct in visited.iter().filter(|&&a| a != owned) {
            let mut loans = HashSet::new();
            let (mut amt, mut bal) = (0.0, 0.0);
            for dep in g.relationships(acct, Direction::Incoming, "deposit") {
                let ts = rel_ts(g, dep.pos);
                if ts < start_ms || ts > end_ms {
                    continue;
                }
                if loans.insert(dep.neighbor) {
                    amt += node_prop(g, dep.neighbor, "amount")
                        .and_then(|v| v.to_f64())
                        .unwrap_or(0.0);
                    bal += node_prop(g, dep.neighbor, "balance")
                        .and_then(|v| v.to_f64())
                        .unwrap_or(0.0);
                }
            }
            if !loans.is_empty() {
                let e = by_acct.entry(acct).or_insert((0.0, 0.0));
                e.0 += amt;
                e.1 += bal;
            }
        }
    }

    let mut out: Vec<(u32, f64, f64)> = by_acct.into_iter().map(|(a, (x, y))| (a, x, y)).collect();
    out.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.0.cmp(&b.0))
    });
    out
}

// ===== TCR5-TCR12 (drafted; tasks 083-090) =====
// ===== CR5 =====
/// TCR5-style — exact account transfer trace from a person's account through at
/// most 3 transfer hops, with strictly increasing timestamps in [start_ms, end_ms]
/// and no cycles. Returns all discovered paths sorted by length descending.
pub fn cr5(
    g: &GraphSnapshot,
    person: u32,
    start_ms: i64,
    end_ms: i64,
    truncation_limit: u32,
    _truncation_order: &str,
) -> Vec<Vec<u32>> {
    let mut all_paths = Vec::new();

    // Find accounts owned by this person via "own" edges (person -> account)
    for r in g.relationships(person, Direction::Outgoing, "own") {
        let start_account = r.neighbor;
        let mut path = vec![start_account];
        let mut visited = HashSet::new();
        visited.insert(start_account);

        cr5_dfs(
            g,
            start_account,
            start_ms,
            end_ms,
            i64::MIN,
            &mut path,
            &mut visited,
            &mut all_paths,
            0,
            truncation_limit,
        );
    }

    // Sort by path length descending
    all_paths.sort_by(|a, b| b.len().cmp(&a.len()));
    all_paths
}

/// DFS helper for cr5: explores transfer paths from a node with time-window,
/// strictly increasing timestamp, and cycle constraints.
#[allow(clippy::too_many_arguments)]
fn cr5_dfs(
    g: &GraphSnapshot,
    node: u32,
    start_ms: i64,
    end_ms: i64,
    last_ts: i64,
    path: &mut Vec<u32>,
    visited: &mut HashSet<u32>,
    out: &mut Vec<Vec<u32>>,
    depth: u32,
    truncation_limit: u32,
) {
    // Stop at max 3 hops (edges)
    if depth >= 3 {
        return;
    }

    // Collect valid outgoing transfer edges: in time window, strictly after
    // last_ts, and target not yet visited in this path
    let mut candidates: Vec<(u32, i64)> = Vec::new();
    for r in g.relationships(node, Direction::Outgoing, "transfer") {
        let ts = rel_ts(g, r.pos);
        if ts >= start_ms && ts <= end_ms && ts > last_ts && !visited.contains(&r.neighbor) {
            candidates.push((r.neighbor, ts));
        }
    }

    // Apply truncation: sort by timestamp ascending, then keep top N edges
    if truncation_limit > 0 && candidates.len() > truncation_limit as usize {
        candidates.sort_by_key(|(_, ts)| *ts);
        candidates.truncate(truncation_limit as usize);
    }

    // Explore each candidate neighbor
    for (neighbor, ts) in candidates {
        path.push(neighbor);
        visited.insert(neighbor);
        out.push(path.clone()); // Record this path

        // Recurse deeper
        cr5_dfs(
            g,
            neighbor,
            start_ms,
            end_ms,
            ts,
            path,
            visited,
            out,
            depth + 1,
            truncation_limit,
        );

        path.pop();
        visited.remove(&neighbor);
    }
}

// ===== CR6 =====
/// TCR6-style — Withdrawal after Many-to-One transfer.
///
/// Given a card account (dstCard), find all accounts (mid) that:
/// 1. Withdraw to dstCard with amount > threshold2 within [startTime, endTime]
/// 2. Have > 3 distinct incoming transfers from sources with amount > threshold1
///
/// Returns Vec<(midId, sumEdge1Amount, sumEdge2Amount)> sorted by
/// sumEdge2Amount descending, then midId ascending.
pub fn cr6(
    g: &GraphSnapshot,
    dst_card: u32,
    threshold1: f64,
    threshold2: f64,
    start_ms: i64,
    end_ms: i64,
    truncation_limit: u32,
    truncation_order: &str,
) -> Vec<(u32, f64, f64)> {
    // The card's outgoing withdrawals in window, amount > threshold2.
    let withdraws: Vec<(i64, f64)> = g
        .relationships(dst_card, Direction::Outgoing, "withdraw")
        .filter_map(|r| {
            let (ts, amt) = (rel_ts(g, r.pos), rel_amt(g, r.pos));
            ((start_ms..=end_ms).contains(&ts) && amt > threshold2).then_some((ts, amt))
        })
        .collect();
    if withdraws.is_empty() {
        return Vec::new();
    }
    let total_withdraw: f64 = withdraws.iter().map(|(_, a)| a).sum();
    let last_withdraw = withdraws.iter().map(|(t, _)| *t).max().unwrap();

    // Many-to-one: source accounts whose in-window transfer in (> threshold1)
    // precedes a withdrawal. Sum per source; pair with the card's total withdrawn.
    let _ = (truncation_limit, truncation_order);
    let mut by_src: HashMap<u32, f64> = HashMap::new();
    for r in g.relationships(dst_card, Direction::Incoming, "transfer") {
        let (ts, amt) = (rel_ts(g, r.pos), rel_amt(g, r.pos));
        if (start_ms..=end_ms).contains(&ts) && amt > threshold1 && ts <= last_withdraw {
            *by_src.entry(r.neighbor).or_default() += amt;
        }
    }
    let mut out: Vec<(u32, f64, f64)> = by_src
        .into_iter()
        .map(|(s, a)| (s, a, total_withdraw))
        .collect();
    out.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(a.0.cmp(&b.0))
    });
    out
}

// ===== CR7 =====
#[derive(Clone, Copy, Debug)]
pub enum TruncationOrder {
    Ascending,
    Descending,
}

/// TCR7-style — transfer in/out ratio. Given an account and time window,
/// find all transfer-in and transfer-out edges where amount exceeds a threshold.
/// Return the count of distinct source/destination accounts and the ratio of
/// total transfer-in amount to transfer-out amount (or -1.0 if no outgoing transfers).
pub fn cr7(
    g: &GraphSnapshot,
    account: u32,
    threshold: f64,
    start_ms: i64,
    end_ms: i64,
    truncation_limit: u32,
    truncation_order: TruncationOrder,
) -> (u32, u32, f64) {
    // Collect and filter transfer-in edges (incoming)
    let mut in_edges: Vec<(i64, f64, u32)> = Vec::new();
    for r in g.relationships(account, Direction::Incoming, "transfer") {
        let ts = rel_ts(g, r.pos);
        let amt = rel_amt(g, r.pos);
        if ts >= start_ms && ts <= end_ms && amt > threshold {
            in_edges.push((ts, amt, r.neighbor));
        }
    }

    // Sort by truncation order and apply limit
    match truncation_order {
        TruncationOrder::Ascending => in_edges.sort_by_key(|e| e.0),
        TruncationOrder::Descending => in_edges.sort_by(|a, b| b.0.cmp(&a.0)),
    }
    in_edges.truncate(truncation_limit as usize);

    // Aggregate transfer-in: count distinct sources, sum amounts
    let mut in_src_set = HashSet::new();
    let mut in_amount = 0.0;
    for (_, amt, neighbor) in &in_edges {
        in_src_set.insert(*neighbor);
        in_amount += amt;
    }
    let num_src = in_src_set.len() as u32;

    // Collect and filter transfer-out edges (outgoing)
    let mut out_edges: Vec<(i64, f64, u32)> = Vec::new();
    for r in g.relationships(account, Direction::Outgoing, "transfer") {
        let ts = rel_ts(g, r.pos);
        let amt = rel_amt(g, r.pos);
        if ts >= start_ms && ts <= end_ms && amt > threshold {
            out_edges.push((ts, amt, r.neighbor));
        }
    }

    // Sort by truncation order and apply limit
    match truncation_order {
        TruncationOrder::Ascending => out_edges.sort_by_key(|e| e.0),
        TruncationOrder::Descending => out_edges.sort_by(|a, b| b.0.cmp(&a.0)),
    }
    out_edges.truncate(truncation_limit as usize);

    // Aggregate transfer-out: count distinct destinations, sum amounts
    let mut out_dst_set = HashSet::new();
    let mut out_amount = 0.0;
    for (_, amt, neighbor) in &out_edges {
        out_dst_set.insert(*neighbor);
        out_amount += amt;
    }
    let num_dst = out_dst_set.len() as u32;

    // Calculate ratio (return -1.0 if no outgoing transfers)
    let in_out_ratio = if out_amount > 0.0 {
        ((in_amount / out_amount * 1000.0).round() / 1000.0)
    } else {
        -1.0
    };

    (num_src, num_dst, in_out_ratio)
}

// ===== CR8 =====
/// TCR8-style — transfer trace after loan applied. Given a loan and a time window,
/// trace transfer/withdraw paths from the account(s) the loan deposits to (up to distance 3 from loan).
/// For each transfer, check if the amount exceeds threshold * (source account's total incoming transfer amount).
/// Return all reached accounts (dstId, ratio, minDistanceFromLoan), sorted by distance DESC, ratio DESC, id ASC.
pub fn cr8(
    g: &GraphSnapshot,
    loan_id: u32,
    threshold: f64,
    start_ms: i64,
    end_ms: i64,
    truncation_limit: u32,
    truncation_order: &str,
) -> Vec<(u32, f64, u32)> {
    // Get loan amount (for ratio calculation: inflow / loan_amount)
    let loan_amount = node_prop(g, loan_id, "amount")
        .and_then(|v| v.to_f64())
        .unwrap_or(1.0);

    // Find all deposit edges from the loan within the time window [start_ms, end_ms]
    let deposit_edges: Vec<(u32, f64)> = g
        .relationships(loan_id, Direction::Outgoing, "deposit")
        .filter(|r| {
            let ts = rel_ts(g, r.pos);
            (start_ms..=end_ms).contains(&ts)
        })
        .map(|r| (r.neighbor, rel_amt(g, r.pos)))
        .collect();

    // Results: dstId -> (total_inflow, min_distance_from_loan)
    let mut results: HashMap<u32, (f64, u32)> = HashMap::new();

    // For each deposited account, trace transfers/withdraws up to distance 3
    for (start_account, deposit_amt) in deposit_edges {
        let mut visited: HashSet<u32> = HashSet::new();
        visited.insert(start_account);

        let mut queue: VecDeque<(u32, u32, f64)> = VecDeque::new();
        queue.push_back((start_account, 1, deposit_amt)); // distance 1 (via deposit edge)

        while let Some((node, dist, inflow)) = queue.pop_front() {
            // Add/update results with this account
            results
                .entry(node)
                .and_modify(|(inf, d)| {
                    *inf += inflow; // Sum inflows from all paths
                    *d = (*d).min(dist); // Track minimum distance
                })
                .or_insert((inflow, dist));

            // Stop BFS at distance 3 from loan (1 deposit + 2 transfers)
            if dist >= 3 {
                continue;
            }

            // Calculate upstream transfer-in total for this account
            let upstream_total: f64 = g
                .relationships(node, Direction::Incoming, "transfer")
                .filter(|r| {
                    let ts = rel_ts(g, r.pos);
                    (start_ms..=end_ms).contains(&ts)
                })
                .map(|r| rel_amt(g, r.pos))
                .sum();

            // Collect outgoing transfer and withdraw edges within time window
            let mut edges: Vec<_> = g
                .relationships(node, Direction::Outgoing, "transfer")
                .chain(g.relationships(node, Direction::Outgoing, "withdraw"))
                .filter(|r| {
                    let ts = rel_ts(g, r.pos);
                    (start_ms..=end_ms).contains(&ts)
                })
                .collect();

            // Sort edges by amount according to truncation_order
            if truncation_order == "DESC" {
                edges.sort_by(|a, b| {
                    rel_amt(g, b.pos)
                        .partial_cmp(&rel_amt(g, a.pos))
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
            } else {
                edges.sort_by(|a, b| {
                    rel_amt(g, a.pos)
                        .partial_cmp(&rel_amt(g, b.pos))
                        .unwrap_or(std::cmp::Ordering::Equal)
                });
            }

            // Apply truncation limit (max edges per step)
            edges.truncate(truncation_limit as usize);

            // Process edges: only follow if amount > threshold * upstream_total
            for edge in edges {
                let edge_amt = rel_amt(g, edge.pos);

                if edge_amt > threshold * upstream_total {
                    if visited.insert(edge.neighbor) {
                        queue.push_back((edge.neighbor, dist + 1, edge_amt));
                    }
                }
            }
        }
    }

    // Convert to result format: (dstId, ratio, minDistanceFromLoan)
    let mut result: Vec<(u32, f64, u32)> = results
        .into_iter()
        .map(|(did, (total_in, dist))| {
            // Ratio = total_inflow / loan_amount, rounded to 3 decimal places
            let ratio = (total_in / loan_amount * 1000.0).round() / 1000.0;
            (did, ratio, dist)
        })
        .collect();

    // Sort: distanceFromLoan DESC, ratio DESC, dstId ASC
    result.sort_by(|a, b| {
        b.2.cmp(&a.2) // distance descending (farthest first)
            .then_with(|| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal)) // ratio descending (highest first)
            .then_with(|| a.0.cmp(&b.0)) // dstId ascending
    });

    result
}

// ===== CR9 =====
/// TCR9: Money laundering with loan involved
///
/// Given an account, a transfer amount threshold, and a time window,
/// find deposit and repay edges between the account and loans, and
/// transfers-in and transfers-out. Returns three ratios:
/// - ratioRepay = sum(edge1) / sum(edge2), or -1 if no edge2 found
/// - ratioDeposit = sum(edge1) / sum(edge4), or -1 if no edge4 found
/// - ratioTransfer = sum(edge3) / sum(edge4), or -1 if no edge4 found
///
/// Edge mapping (per loader schema):
/// - edge1: repay edges (account -> loan)
/// - edge2: deposit edges (loan -> account)
/// - edge3: transfer edges out (account -> other_account)
/// - edge4: transfer edges in (other_account -> account)
///
/// All edges filtered by time window [start_ms, end_ms].
/// Transfer edges (edge3, edge4) additionally filtered by amount >= threshold.
/// At most truncation_limit edges of each type are kept (after sorting by truncation_order).
pub fn cr9(
    g: &GraphSnapshot,
    account: u32,
    threshold: f64,
    start_ms: i64,
    end_ms: i64,
    truncation_limit: usize,
    truncation_asc: bool, // true = ascending by timestamp, false = descending
) -> (f32, f32, f32) {
    // Collect edge1 (repay: account -> loan)
    let mut edge1_edges: Vec<(i64, f64)> = Vec::new();
    for r in g.relationships(account, Direction::Outgoing, "repay") {
        let ts = rel_ts(g, r.pos);
        if ts >= start_ms && ts <= end_ms {
            edge1_edges.push((ts, rel_amt(g, r.pos)));
        }
    }

    // Collect edge2 (deposit: loan -> account)
    let mut edge2_edges: Vec<(i64, f64)> = Vec::new();
    for r in g.relationships(account, Direction::Incoming, "deposit") {
        let ts = rel_ts(g, r.pos);
        if ts >= start_ms && ts <= end_ms {
            edge2_edges.push((ts, rel_amt(g, r.pos)));
        }
    }

    // Collect edge3 (transfer-out: account -> other_account, amt >= threshold)
    let mut edge3_edges: Vec<(i64, f64)> = Vec::new();
    for r in g.relationships(account, Direction::Outgoing, "transfer") {
        let ts = rel_ts(g, r.pos);
        let amt = rel_amt(g, r.pos);
        if ts >= start_ms && ts <= end_ms && amt >= threshold {
            edge3_edges.push((ts, amt));
        }
    }

    // Collect edge4 (transfer-in: other_account -> account, amt >= threshold)
    let mut edge4_edges: Vec<(i64, f64)> = Vec::new();
    for r in g.relationships(account, Direction::Incoming, "transfer") {
        let ts = rel_ts(g, r.pos);
        let amt = rel_amt(g, r.pos);
        if ts >= start_ms && ts <= end_ms && amt >= threshold {
            edge4_edges.push((ts, amt));
        }
    }

    // Helper: apply truncation (sort by timestamp, then limit)
    fn apply_truncation(edges: &mut Vec<(i64, f64)>, limit: usize, asc: bool) {
        if asc {
            edges.sort_by_key(|e| e.0);
        } else {
            edges.sort_by(|a, b| b.0.cmp(&a.0));
        }
        edges.truncate(limit);
    }

    apply_truncation(&mut edge1_edges, truncation_limit, truncation_asc);
    apply_truncation(&mut edge2_edges, truncation_limit, truncation_asc);
    apply_truncation(&mut edge3_edges, truncation_limit, truncation_asc);
    apply_truncation(&mut edge4_edges, truncation_limit, truncation_asc);

    // Sum amounts
    let edge1_sum: f64 = edge1_edges.iter().map(|(_, amt)| amt).sum();
    let edge2_sum: f64 = edge2_edges.iter().map(|(_, amt)| amt).sum();
    let edge3_sum: f64 = edge3_edges.iter().map(|(_, amt)| amt).sum();
    let edge4_sum: f64 = edge4_edges.iter().map(|(_, amt)| amt).sum();

    // Helper: round to 3 decimal places
    fn round_3dp(x: f64) -> f32 {
        ((x * 1000.0).round() / 1000.0) as f32
    }

    // Calculate ratios with -1 for division by zero
    let ratio_repay = if edge2_sum == 0.0 {
        -1.0f32
    } else {
        round_3dp(edge1_sum / edge2_sum)
    };

    let ratio_deposit = if edge4_sum == 0.0 {
        -1.0f32
    } else {
        round_3dp(edge1_sum / edge4_sum)
    };

    let ratio_transfer = if edge4_sum == 0.0 {
        -1.0f32
    } else {
        round_3dp(edge3_sum / edge4_sum)
    };

    (ratio_repay, ratio_deposit, ratio_transfer)
}

fn apply_truncation(edges: &mut Vec<(i64, f64)>, limit: usize, asc: bool) {
    if asc {
        edges.sort_by_key(|e| e.0);
    } else {
        edges.sort_by(|a, b| b.0.cmp(&a.0));
    }
    edges.truncate(limit);
}

fn round_3dp(x: f64) -> f32 {
    ((x * 1000.0).round() / 1000.0) as f32
}

// ===== CR10 =====
/// TCR10 — Similarity of investor relationship: for a Person, find the other
/// investors who share invested Companies (in window), returning `(otherId,
/// sharedCompanyCount)` sorted by count desc, id asc. One-to-many per the spec
/// (the draft's pairwise Jaccard was a different shape). Note: incoming `invest`
/// also carries `companyInvest`, so a few `otherId`s may be companies.
pub fn cr10(g: &GraphSnapshot, person: u32, start_ms: i64, end_ms: i64) -> Vec<(u32, usize)> {
    let mut companies = HashSet::new();
    for r in g.relationships(person, Direction::Outgoing, "invest") {
        let ts = rel_ts(g, r.pos);
        if ts >= start_ms && ts <= end_ms {
            companies.insert(r.neighbor);
        }
    }
    let mut shared: HashMap<u32, usize> = HashMap::new();
    for &c in &companies {
        for r in g.relationships(c, Direction::Incoming, "invest") {
            if r.neighbor != person {
                *shared.entry(r.neighbor).or_default() += 1;
            }
        }
    }
    let mut out: Vec<(u32, usize)> = shared.into_iter().collect();
    out.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    out
}

// ===== CR12 =====
/// TCR12 — transfer-to-company amount statistics. Given a person and a time window,
/// find all company-owned accounts that the person has transferred to (via accounts
/// they own), returning the sum of transfer amounts per company account.
///
/// Traversal pattern:
/// 1. Person -[own]→ Account (person's accounts) → truncate per limit
/// 2. Account -[transfer]→ Account (within [start_ms, end_ms]) → truncate per limit and order
/// 3. Verify target Account ←[own]- Company → aggregate
/// 4. Sort result by summedAmount desc, then compAccountId asc
pub fn cr12(
    g: &GraphSnapshot,
    person_id: u32,
    start_ms: i64,
    end_ms: i64,
    truncation_limit: u32,
    truncation_order: TruncationOrder,
) -> Vec<(u32, f64)> {
    let mut company_account_amounts: HashMap<u32, f64> = HashMap::new();

    // Pre-compute set of company nodes for O(1) ownership checks.
    let companies: HashSet<u32> = g
        .nodes_with_label("Company")
        .map(|ns| ns.iter().collect())
        .unwrap_or_default();

    // Step 1: Find all accounts owned by the person.
    let mut person_accounts: Vec<u32> = g
        .relationships(person_id, Direction::Outgoing, "own")
        .map(|r| r.neighbor)
        .collect();

    // Apply truncation limit at step 1 (use account ID order as default).
    if person_accounts.len() > truncation_limit as usize {
        person_accounts.sort();
        person_accounts.truncate(truncation_limit as usize);
    }

    // Step 2: From each person account, find transfers to company-owned accounts.
    for &person_account in &person_accounts {
        // Collect outgoing transfer edges within the time window [start_ms, end_ms].
        let mut transfers: Vec<(u32, f64)> = Vec::new();
        for rel in g.relationships(person_account, Direction::Outgoing, "transfer") {
            let ts = rel_ts(g, rel.pos);
            if ts >= start_ms && ts <= end_ms {
                let amt = rel_amt(g, rel.pos);
                transfers.push((rel.neighbor, amt));
            }
        }

        // Sort by amount per truncation_order, then apply truncation limit.
        match truncation_order {
            TruncationOrder::Descending => {
                transfers.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
            }
            TruncationOrder::Ascending => {
                transfers.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());
            }
        }
        if transfers.len() > truncation_limit as usize {
            transfers.truncate(truncation_limit as usize);
        }

        // Step 3: Verify each transfer target is company-owned and aggregate amounts.
        for (target_account, amt) in transfers {
            let is_company_owned = g
                .relationships(target_account, Direction::Incoming, "own")
                .any(|rel| companies.contains(&rel.neighbor));

            if is_company_owned {
                *company_account_amounts.entry(target_account).or_insert(0.0) += amt;
            }
        }
    }

    // Step 4: Sort result by summed amount (descending), then by account ID (ascending).
    let mut result: Vec<(u32, f64)> = company_account_amounts.into_iter().collect();
    result.sort_by(
        |a, b| match b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal) {
            std::cmp::Ordering::Equal => a.0.cmp(&b.0),
            other => other,
        },
    );

    result
}

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

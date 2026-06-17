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

use std::collections::HashMap;
use std::path::Path;
use std::time::Instant;

use rustychickpeas_core::{GraphBuilder, GraphSnapshot, PropertyValue};

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
    for_each_csv(&raw.join("medium"), &["id"], |v| {
        let nid = next;
        next += 1;
        med.insert(v[0].parse().unwrap_or(0), nid);
        b.add_node(Some(nid), &["Medium"]).unwrap();
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

/// TCR1-style — given an account, trace all `transfer` paths (≤k hops) reaching
/// it inside `[start_ms, end_ms]`, returning the touched accounts. The window
/// filter reads each edge's timestamp during traversal.
pub fn trace_transfers_in(
    _g: &GraphSnapshot,
    _account: u32,
    _start_ms: i64,
    _end_ms: i64,
    _max_hops: u32,
) -> Vec<u32> {
    todo!("bounded reverse BFS over `transfer`, filter edge timestamp in window")
}

/// TCR8-style — detect fund-transfer cycles through a given account where each
/// hop is strictly increasing in time and the amount exceeds a threshold.
pub fn transfer_cycles(
    _g: &GraphSnapshot,
    _account: u32,
    _min_amount: f64,
    _window_ms: i64,
) -> Vec<Vec<u32>> {
    todo!("DFS for time-ordered cycles; prune on amount and monotone timestamp")
}

/// TCR3-style — shortest `transfer` path between two accounts within a window;
/// edge weight is unit (hop count). Reuses `g.dijkstra`, with the weight closure
/// rejecting out-of-window edges via the relationship accessor's `pos`.
pub fn shortest_transfer_path(
    _g: &GraphSnapshot,
    _src: u32,
    _dst: u32,
    _start_ms: i64,
    _end_ms: i64,
) -> i64 {
    todo!("g.dijkstra over `transfer` with in-window guard in the weight closure")
}

/// TCR11-style — sum a person's loan exposure by walking `guarantee` chains and
/// the `apply`/`own` edges out to the loans they are on the hook for.
pub fn guarantee_exposure(_g: &GraphSnapshot, _person: u32) -> f64 {
    todo!("traverse guarantee chain -> apply/own -> loan, accumulate amounts")
}

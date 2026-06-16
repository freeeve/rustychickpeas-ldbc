//! SCAFFOLD — LDBC FinBench (Financial Benchmark). Not yet wired into the build.
//!
//! Inert until `tasks/001` extracts `src/lib.rs` and `tasks/008` makes a
//! `src/bin/finbench.rs` binary. FinBench is a different schema from SNB:
//! Account / Person / Company / Medium / Loan nodes, and time-stamped, amount-
//! weighted edges — `transfer`, `withdraw`, `deposit`, `repay`, `guarantee`,
//! `invest`, `signIn`, `own`, `apply`. Needs its own loader (`tasks/007`).
//!
//! The read workload is transaction tracing: temporal fund-flow paths, transfer
//! cycles inside a time window, blocked-account propagation. This plays to the
//! same edge-property-during-traversal capability Q11 drove (per-edge
//! `creationDate` / `amount` read via the relationship accessor's CSR position).
//! The few representative complex reads below are filled in by `tasks/008`.

use rustychickpeas_core::GraphSnapshot;

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

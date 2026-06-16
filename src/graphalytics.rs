//! SCAFFOLD — LDBC Graphalytics algorithms. Not yet wired into the build.
//!
//! Inert until `tasks/001` extracts `src/lib.rs` and `tasks/006` makes a
//! `src/bin/graphalytics.rs` runner. Graphalytics is pure topology: load a
//! `.v` (vertices) + `.e` (edges) dataset, run an algorithm, and compare the
//! output to the dataset's reference file (exact for BFS/WCC/CDLP, epsilon for
//! PR/LCC/SSSP). The six algorithms below are the full Graphalytics set.
//!
//! We already have two of them in `main.rs`:
//!   * SSSP — `g.dijkstra(source, Direction::Both, types, None, weight)`
//!   * WCC  — the connected-components pass behind Q3/Q4/Q12
//! so this module is mostly BFS + PageRank + CDLP + LCC, plus the loader and the
//! reference-output validator.

use rustychickpeas_core::GraphSnapshot;

/// Breadth-first levels from `source`; unreachable vertices report -1, per the
/// Graphalytics BFS output spec.
pub fn bfs(_g: &GraphSnapshot, _source: u32) -> Vec<i64> {
    todo!("frontier BFS over the CSR adjacency, level per vertex")
}

/// PageRank after a fixed iteration count (Graphalytics fixes `max_iterations`
/// and the damping factor per run; no convergence test). Compared with epsilon.
pub fn pagerank(_g: &GraphSnapshot, _damping: f64, _iterations: u32) -> Vec<f64> {
    todo!("iterate rank push over out-neighbours; handle dangling vertices")
}

/// Weakly connected components — component id per vertex. Reuse the
/// connected-components pass already behind Q3/Q4/Q12.
pub fn wcc(_g: &GraphSnapshot) -> Vec<u32> {
    todo!("reuse existing union-find / label-propagation CC from main.rs")
}

/// Community detection by label propagation, fixed iteration count. Output is a
/// community label per vertex (compared exactly after canonical relabelling).
pub fn cdlp(_g: &GraphSnapshot, _iterations: u32) -> Vec<u32> {
    todo!("synchronous label propagation, deterministic lowest-label tie-break")
}

/// Local clustering coefficient per vertex (fraction of neighbour pairs that are
/// themselves adjacent). Compared with epsilon.
pub fn lcc(_g: &GraphSnapshot) -> Vec<f64> {
    todo!("per-vertex neighbour-pair adjacency count over the CSR / RoaringBitmap")
}

/// Single-source shortest paths with unit or stored edge weights; thin wrapper
/// over the existing `g.dijkstra`. Unreachable vertices report +inf.
pub fn sssp(_g: &GraphSnapshot, _source: u32) -> Vec<f64> {
    todo!("g.dijkstra(source, dir, edge_types, None, weight) -> per-vertex distance")
}

// Dataset I/O + validation (tasks/005–006): parse `<name>.v` / `<name>.e` into a
// GraphBuilder, read `<name>.properties` for the per-algorithm parameters, and
// diff against `<name>-<ALGO>` reference output with the spec's tolerance.

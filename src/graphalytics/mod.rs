//! LDBC Graphalytics — the six benchmark algorithms (BFS, PR, WCC, CDLP, LCC,
//! SSSP) over a loaded `.v`/`.e` dataset, plus (to come) the dataset loader and
//! the reference-output validator. Implemented to the spec v1.0.x §2.3.
//!
//! Build scaffold: BFS/SSSP are thin wrappers over core (`bfs_distances` /
//! `dijkstra`); WCC/PageRank/CDLP/LCC are filled by tasks 124/123/125/126.
//! Edges are a single `e` type with a `weight` f64 property; algorithms take a
//! `directed` flag (forward edges = outgoing for directed, both for undirected).

use rustychickpeas_core::{Direction, GraphSnapshot};

/// Forward edge direction (BFS/SSSP/out-neighbours): outgoing for a directed
/// graph, both for an undirected one (whose edges are stored once).
pub(crate) fn fwd(directed: bool) -> Direction {
    if directed {
        Direction::Outgoing
    } else {
        Direction::Both
    }
}

/// Breadth-first depth from `source` over forward edges; unreachable vertices get
/// `i64::MAX` (9223372036854775807), per the spec.
pub fn bfs(g: &GraphSnapshot, source: u32, directed: bool) -> Vec<i64> {
    let dist = g.bfs_distances(source, fwd(directed), &[] as &[&str], None);
    (0..g.node_count()).map(|v| dist.get(&v).map_or(i64::MAX, |&d| d as i64)).collect()
}

/// Single-source shortest paths over forward edges (`weight` edge property when
/// `weighted`, else unit); unreachable vertices get `f64::INFINITY`.
pub fn sssp(g: &GraphSnapshot, source: u32, directed: bool, weighted: bool) -> Vec<f64> {
    let sp = g.dijkstra(source, fwd(directed), &[] as &[&str], None, |_from, rel| {
        if weighted {
            g.relationship_property(rel.pos, "weight").and_then(|v| v.to_f64()).unwrap_or(1.0)
        } else {
            1.0
        }
    });
    (0..g.node_count()).map(|v| sp.distance(v).unwrap_or(f64::INFINITY)).collect()
}

/// Weakly connected components — component label per vertex (task 124).
pub fn wcc(_g: &GraphSnapshot) -> Vec<u32> {
    todo!("task 124: union-find / BFS over Direction::Both; label = min vertex id")
}

/// PageRank after a fixed iteration count (task 123).
pub fn pagerank(_g: &GraphSnapshot, _directed: bool, _damping: f64, _iterations: u32) -> Vec<f64> {
    todo!("task 123: PR0=1/|V|; pull from in-neighbours / out-degree; redistribute sinks")
}

/// Community detection by synchronous label propagation (task 125).
pub fn cdlp(_g: &GraphSnapshot, _directed: bool, _iterations: u32) -> Vec<u32> {
    todo!("task 125: L0=id; most frequent in+out neighbour label, smallest on tie")
}

/// Local clustering coefficient per vertex (task 126).
pub fn lcc(_g: &GraphSnapshot, _directed: bool) -> Vec<f64> {
    todo!("task 126: edges among the undirected neighbour set / |N|(|N|-1)")
}

//! LDBC Graphalytics — the six benchmark algorithms (BFS, PR, WCC, CDLP, LCC,
//! SSSP) over a loaded `.v`/`.e` dataset, plus (to come) the dataset loader and
//! the reference-output validator. Implemented to the spec v1.0.x §2.3.
//!
//! BFS/SSSP are thin wrappers over core (`bfs_distances` / `dijkstra`);
//! WCC/PageRank/CDLP/LCC are implemented directly over the snapshot adjacency.
//! Edges are a single `e` type with a `weight` f64 property; algorithms take a
//! `directed` flag (forward edges = outgoing for directed, both for undirected).

use std::collections::{HashMap, HashSet};

use rustychickpeas_core::{Direction, GraphSnapshot};

pub mod load;
pub mod validate;

pub use load::{load, Dataset, Params};

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

/// Weakly connected components: each vertex's label is the smallest vertex id in
/// its component, found by flooding undirected (`Direction::Both`) edges. Vertices
/// are swept in ascending id order, so the first-reached vertex of a component is
/// its minimum and becomes the label.
pub fn wcc(g: &GraphSnapshot) -> Vec<u32> {
    let n = g.node_count();
    let mut comp = vec![u32::MAX; n as usize];
    let mut stack: Vec<u32> = Vec::new();
    for s in 0..n {
        if comp[s as usize] != u32::MAX {
            continue;
        }
        comp[s as usize] = s;
        stack.push(s);
        while let Some(v) = stack.pop() {
            for u in g.neighbors(v, Direction::Both) {
                if comp[u as usize] == u32::MAX {
                    comp[u as usize] = s;
                    stack.push(u);
                }
            }
        }
    }
    comp
}

/// PageRank after `iterations` synchronous updates with damping `damping`:
/// `PR0(v) = 1/|V|`, then `PRi(v) = (1-d)/|V| + d*(Σ_{u∈Nin(v)} PRi-1(u)/|Nout(u)|
/// + Σ_{sink w} PRi-1(w)/|V|)`. Sinks (out-degree 0) redistribute their rank
/// uniformly. Forward edges are outgoing (directed) or both (undirected).
pub fn pagerank(g: &GraphSnapshot, directed: bool, damping: f64, iterations: u32) -> Vec<f64> {
    let n = g.node_count() as usize;
    if n == 0 {
        return Vec::new();
    }
    let nf = n as f64;
    let out = fwd(directed);
    let outdeg: Vec<u32> = (0..n as u32).map(|v| g.neighbors(v, out).count() as u32).collect();
    let mut pr = vec![1.0 / nf; n];
    for _ in 0..iterations {
        let dangling: f64 = (0..n).filter(|&v| outdeg[v] == 0).map(|v| pr[v]).sum();
        // Push each non-sink's share along its forward edges, accumulating the
        // pull into each in-neighbour.
        let mut next = vec![0.0_f64; n];
        for u in 0..n as u32 {
            let d = outdeg[u as usize];
            if d == 0 {
                continue;
            }
            let share = pr[u as usize] / d as f64;
            for w in g.neighbors(u, out) {
                next[w as usize] += share;
            }
        }
        let base = (1.0 - damping) / nf + damping * dangling / nf;
        for v in next.iter_mut() {
            *v = base + damping * *v;
        }
        pr = next;
    }
    pr
}

/// Community detection by synchronous label propagation: `L0(v) = v`, then each
/// vertex adopts the most frequent label among its neighbours (incoming and
/// outgoing tallied separately for directed graphs, so a mutual edge counts the
/// label twice; each neighbour once for undirected), smallest label breaking
/// ties. A vertex with no neighbours keeps its label. Runs `iterations` rounds.
pub fn cdlp(g: &GraphSnapshot, directed: bool, iterations: u32) -> Vec<u32> {
    let n = g.node_count();
    let mut labels: Vec<u32> = (0..n).collect();
    let mut counts: HashMap<u32, u32> = HashMap::new();
    for _ in 0..iterations {
        let mut next = labels.clone();
        for v in 0..n {
            counts.clear();
            if directed {
                for u in g.neighbors(v, Direction::Outgoing) {
                    *counts.entry(labels[u as usize]).or_insert(0) += 1;
                }
                for u in g.neighbors(v, Direction::Incoming) {
                    *counts.entry(labels[u as usize]).or_insert(0) += 1;
                }
            } else {
                for u in g.neighbors(v, Direction::Both) {
                    *counts.entry(labels[u as usize]).or_insert(0) += 1;
                }
            }
            // Highest frequency, smallest label on a tie.
            if let Some((&label, _)) = counts.iter().max_by(|a, b| a.1.cmp(b.1).then(b.0.cmp(a.0))) {
                next[v as usize] = label;
            }
        }
        labels = next;
    }
    labels
}

/// Local clustering coefficient: for each vertex `v` with undirected neighbour set
/// `N(v)` (each neighbour once, self excluded), `0` if `|N(v)| <= 1` else the
/// number of forward edges running between members of `N(v)` divided by
/// `|N(v)|*(|N(v)|-1)`. Forward edges are outgoing (directed) or both (undirected).
pub fn lcc(g: &GraphSnapshot, directed: bool) -> Vec<f64> {
    let n = g.node_count();
    let out = fwd(directed);
    let mut result = vec![0.0_f64; n as usize];
    for v in 0..n {
        let mut nbrs: Vec<u32> = g.neighbors(v, Direction::Both).filter(|&u| u != v).collect();
        nbrs.sort_unstable();
        nbrs.dedup();
        let k = nbrs.len();
        if k <= 1 {
            continue;
        }
        let nset: HashSet<u32> = nbrs.iter().copied().collect();
        let mut edges = 0u64;
        for &u in &nbrs {
            for w in g.neighbors(u, out) {
                if w != u && nset.contains(&w) {
                    edges += 1;
                }
            }
        }
        result[v as usize] = edges as f64 / (k as f64 * (k as f64 - 1.0));
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustychickpeas_core::GraphBuilder;

    /// Build a graph of `n` vertices (labelled `V`) wired by the given `e` edges.
    fn build(n: u32, edges: &[(u32, u32)]) -> GraphSnapshot {
        let mut b = GraphBuilder::new(Some(n as usize), Some(edges.len()));
        for i in 0..n {
            b.add_node(Some(i), &["V"]).unwrap();
        }
        for &(u, v) in edges {
            b.add_relationship(u, v, "e").unwrap();
        }
        b.finalize(None)
    }

    /// As [`build`], but each edge carries a `weight` f64 property for SSSP.
    fn build_weighted(n: u32, edges: &[(u32, u32, f64)]) -> GraphSnapshot {
        let mut b = GraphBuilder::new(Some(n as usize), Some(edges.len()));
        for i in 0..n {
            b.add_node(Some(i), &["V"]).unwrap();
        }
        for &(u, v, w) in edges {
            b.add_relationship(u, v, "e").unwrap();
            b.set_relationship_prop_f64(u, v, "e", "weight", w);
        }
        b.finalize(None)
    }

    #[test]
    fn bfs_depths_and_unreachable() {
        // 0->1->2 reachable at depths 0,1,2; node 3 is isolated -> i64::MAX.
        let g = build(4, &[(0, 1), (1, 2)]);
        assert_eq!(bfs(&g, 0, true), vec![0, 1, 2, i64::MAX]);
    }

    #[test]
    fn sssp_weighted_shortest_and_unreachable() {
        // 0->1 (2) ->2 (3) beats the direct 0->2 (10); node 3 unreachable.
        let g = build_weighted(4, &[(0, 1, 2.0), (1, 2, 3.0), (0, 2, 10.0)]);
        let d = sssp(&g, 0, true, true);
        assert_eq!(d[0], 0.0);
        assert_eq!(d[1], 2.0);
        assert_eq!(d[2], 5.0);
        assert_eq!(d[3], f64::INFINITY);
    }

    #[test]
    fn wcc_two_components_label_min_id() {
        // {0,1,2} via 0->1->2 and {3,4} via 3->4; weak connectivity ignores direction.
        let g = build(5, &[(0, 1), (1, 2), (3, 4)]);
        assert_eq!(wcc(&g), vec![0, 0, 0, 3, 3]);
    }

    #[test]
    fn pagerank_uniform_on_directed_cycle() {
        // A 3-cycle has no sinks and is symmetric, so rank stays uniform at 1/3.
        let g = build(3, &[(0, 1), (1, 2), (2, 0)]);
        let pr = pagerank(&g, true, 0.85, 30);
        for p in &pr {
            assert!((p - 1.0 / 3.0).abs() < 1e-9, "{p}");
        }
        assert!((pr.iter().sum::<f64>() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn pagerank_redistributes_sink_rank() {
        // 0 -> 1 with 1 a sink; one iteration at d=0.85, hand-computed.
        let g = build(2, &[(0, 1)]);
        let pr = pagerank(&g, true, 0.85, 1);
        assert!((pr[0] - 0.2875).abs() < 1e-9, "{}", pr[0]);
        assert!((pr[1] - 0.7125).abs() < 1e-9, "{}", pr[1]);
        assert!((pr.iter().sum::<f64>() - 1.0).abs() < 1e-9);
    }

    #[test]
    fn cdlp_triangle_converges_to_min_label() {
        // Undirected triangle: every vertex collapses to the smallest label (0).
        let g = build(3, &[(0, 1), (1, 2), (2, 0)]);
        assert_eq!(cdlp(&g, false, 2), vec![0, 0, 0]);
    }

    #[test]
    fn lcc_triangle_with_pendant() {
        // Triangle 0-1-2 plus pendant 0-3: v0 over {1,2,3}=2/6, v1=v2=1, v3=0.
        let g = build(4, &[(0, 1), (1, 2), (2, 0), (0, 3)]);
        let coeffs = lcc(&g, false);
        assert!((coeffs[0] - 1.0 / 3.0).abs() < 1e-9, "{}", coeffs[0]);
        assert!((coeffs[1] - 1.0).abs() < 1e-9, "{}", coeffs[1]);
        assert!((coeffs[2] - 1.0).abs() < 1e-9, "{}", coeffs[2]);
        assert_eq!(coeffs[3], 0.0);
    }
}

//! LDBC Graphalytics — the six benchmark algorithms (BFS, PR, WCC, CDLP, LCC,
//! SSSP) over a loaded `.v`/`.e` dataset, plus the dataset loader ([`load`]) and
//! the reference-output validator ([`validate`]). Implemented to the spec v1.0.x §2.3.
//!
//! SSSP wraps core's `dijkstra`; the other five are implemented directly over the
//! snapshot adjacency. Edges are a single `e` type with a `weight` f64 property;
//! algorithms take a `directed` flag (forward edges = outgoing for directed, both
//! for undirected).

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
/// `i64::MAX` (9223372036854775807), per the spec. Level-synchronous BFS over a
/// dense distance array with two reused frontier buffers -- avoids the per-call
/// `HashMap` that `bfs_distances` materialises for a graph reaching millions of
/// vertices.
pub fn bfs(g: &GraphSnapshot, source: u32, directed: bool) -> Vec<i64> {
    let n = g.node_count();
    let dir = fwd(directed);
    let mut dist = vec![i64::MAX; n as usize];
    if source >= n {
        return dist;
    }
    dist[source as usize] = 0;
    // Single FIFO queue (the growing visited list, read via `head`) rather than two
    // level frontiers: allocated once at the reached-set bound instead of
    // reallocating per level. Each vertex's depth is its parent's plus one.
    let mut queue: Vec<u32> = Vec::with_capacity(n as usize);
    queue.push(source);
    let mut head = 0;
    while head < queue.len() {
        let u = queue[head];
        head += 1;
        let du = dist[u as usize];
        for w in g.neighbors(u, dir) {
            if dist[w as usize] == i64::MAX {
                dist[w as usize] = du + 1;
                queue.push(w);
            }
        }
    }
    dist
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
    let mut next = vec![0.0_f64; n];
    for _ in 0..iterations {
        let dangling: f64 = (0..n).filter(|&v| outdeg[v] == 0).map(|v| pr[v]).sum();
        // Push each non-sink's share along its forward edges, accumulating the
        // pull into each in-neighbour. `next` is reused (zeroed in place) rather
        // than reallocated per iteration.
        next.iter_mut().for_each(|x| *x = 0.0);
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
        std::mem::swap(&mut pr, &mut next);
    }
    pr
}

/// Community detection by synchronous label propagation: `L0(v) = v`, then each
/// vertex adopts the most frequent label among its neighbours (incoming and
/// outgoing tallied separately for directed graphs, so a mutual edge counts the
/// label twice; each neighbour once for undirected), smallest label breaking
/// ties. A vertex with no neighbours keeps its label. Runs `iterations` rounds.
pub fn cdlp(g: &GraphSnapshot, directed: bool, iterations: u32) -> Vec<u32> {
    let init: Vec<u32> = (0..g.node_count()).collect();
    cdlp_seeded(g, directed, iterations, &init)
}

/// [`cdlp`] seeded with explicit initial labels (`init[node]` is `L0(node)`). Seed
/// with original vertex ids when the output must match a vertex-id-keyed reference:
/// the "smallest label" tie-break then operates in vertex-id space too, so the
/// result is faithful regardless of how dense node ids map to vertex ids.
pub fn cdlp_seeded(g: &GraphSnapshot, directed: bool, iterations: u32, init: &[u32]) -> Vec<u32> {
    let n = g.node_count();
    let mut cur: Vec<u32> = init.to_vec();
    let mut nxt: Vec<u32> = vec![0; n as usize];
    let mut buf: Vec<u32> = Vec::new();
    for _ in 0..iterations {
        for v in 0..n {
            // Gather neighbour labels (in+out for directed, so a mutual edge counts
            // the label twice; each neighbour once for undirected), then pick the
            // winner by sorting and scanning equal-label runs -- no per-vertex
            // hashing, and the buffer is reused across vertices.
            buf.clear();
            if directed {
                buf.extend(g.neighbors(v, Direction::Outgoing).map(|u| cur[u as usize]));
                buf.extend(g.neighbors(v, Direction::Incoming).map(|u| cur[u as usize]));
            } else {
                buf.extend(g.neighbors(v, Direction::Both).map(|u| cur[u as usize]));
            }
            buf.sort_unstable();
            // Default to the current label (a vertex with no neighbours keeps it).
            // The runs are visited in ascending label order and we replace only on a
            // strictly higher count, so the smallest label wins ties.
            let mut best_label = cur[v as usize];
            let mut best_count = 0usize;
            let mut i = 0;
            while i < buf.len() {
                let lab = buf[i];
                let mut j = i + 1;
                while j < buf.len() && buf[j] == lab {
                    j += 1;
                }
                if j - i > best_count {
                    best_count = j - i;
                    best_label = lab;
                }
                i = j;
            }
            nxt[v as usize] = best_label;
        }
        std::mem::swap(&mut cur, &mut nxt);
    }
    cur
}

/// Local clustering coefficient: for each vertex `v` with undirected neighbour set
/// `N(v)` (each neighbour once, self excluded), `0` if `|N(v)| <= 1` else the
/// number of forward edges running between members of `N(v)` divided by
/// `|N(v)|*(|N(v)|-1)`. Forward edges are outgoing (directed) or both (undirected).
pub fn lcc(g: &GraphSnapshot, directed: bool) -> Vec<f64> {
    let n = g.node_count();
    let out = fwd(directed);
    let mut result = vec![0.0_f64; n as usize];

    // Sorted forward-adjacency (CSR), built once, so the triangle count can probe
    // the smaller side of each intersection: scan u's out-list when it is short,
    // else binary-search it. Without this a single mega-hub u that sits in many
    // N(v) has its whole out-list rescanned every time.
    let mut off = vec![0u32; n as usize + 1];
    for v in 0..n {
        off[v as usize + 1] = off[v as usize] + g.neighbors(v, out).count() as u32;
    }
    let mut adj = vec![0u32; off[n as usize] as usize];
    for v in 0..n {
        let s = off[v as usize] as usize;
        let mut p = s;
        for w in g.neighbors(v, out) {
            adj[p] = w;
            p += 1;
        }
        adj[s..p].sort_unstable();
    }

    // Membership of N(v) as a bit set: ~1/32 the size of a u32 marker array and
    // cache-resident. Set each neighbour's bit while building N(v), test it in the
    // scan branch, then clear exactly those bits (via `nbrs`) before the next
    // vertex -- so no per-vertex O(n) clear and no per-vertex allocation.
    let mut mark = vec![0u64; (n as usize + 63) / 64];
    let mut nbrs: Vec<u32> = Vec::new();
    for v in 0..n {
        nbrs.clear();
        for u in g.neighbors(v, Direction::Both) {
            let marked = mark[(u >> 6) as usize] >> (u & 63) & 1 != 0;
            if u != v && !marked {
                mark[(u >> 6) as usize] |= 1u64 << (u & 63);
                nbrs.push(u);
            }
        }
        let k = nbrs.len();
        if k >= 2 {
            let mut edges = 0u64;
            for &u in &nbrs {
                let uo = &adj[off[u as usize] as usize..off[u as usize + 1] as usize];
                if uo.len() <= k {
                    // Short out-list: scan it, testing N(v) membership by bit.
                    for &w in uo {
                        if w != u && mark[(w >> 6) as usize] >> (w & 63) & 1 != 0 {
                            edges += 1;
                        }
                    }
                } else {
                    // High-degree u: iterate the smaller neighbour set and
                    // binary-search u's out-list rather than scanning it.
                    for &w in &nbrs {
                        if w != u && uo.binary_search(&w).is_ok() {
                            edges += 1;
                        }
                    }
                }
            }
            result[v as usize] = edges as f64 / (k as f64 * (k as f64 - 1.0));
        }
        // Reset N(v)'s bits for the next vertex (including the k < 2 case).
        for &u in &nbrs {
            mark[(u >> 6) as usize] &= !(1u64 << (u & 63));
        }
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
    fn cdlp_seeded_runs_in_seed_label_space() {
        // Seeded with vertex-id-like labels, the triangle collapses to the
        // smallest seed (10) — the tie-break operates in the seed's value space.
        let g = build(3, &[(0, 1), (1, 2), (2, 0)]);
        assert_eq!(cdlp_seeded(&g, false, 2, &[10, 20, 30]), vec![10, 10, 10]);
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

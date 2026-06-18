//! LDBC Graphalytics — the six benchmark algorithms (BFS, PR, WCC, CDLP, LCC,
//! SSSP) over a loaded `.v`/`.e` dataset, plus the dataset loader ([`load`]) and
//! the reference-output validator ([`validate`]). Implemented to the spec v1.0.x §2.3.
//!
//! SSSP wraps core's `dijkstra`; the other five are implemented directly over the
//! snapshot adjacency. Rels are a single `e` type with a `weight` f64 property;
//! algorithms take a `directed` flag (forward rels = outgoing for directed, both
//! for undirected).

use rustychickpeas_core::{Direction, GraphSnapshot, PropExt};

pub mod load;
pub mod validate;

pub use load::{load, Dataset, Params};

/// Forward rel direction (BFS/SSSP/out-neighbours): outgoing for a directed
/// graph, both for an undirected one (whose rels are stored once).
pub(crate) fn fwd(directed: bool) -> Direction {
    if directed {
        Direction::Outgoing
    } else {
        Direction::Both
    }
}

/// Breadth-first depth from `source` over forward rels; unreachable nodes get
/// `i64::MAX` (9223372036854775807), per the spec. Level-synchronous BFS over a
/// dense distance array with two reused frontier buffers -- avoids the per-call
/// `HashMap` that `bfs_distances` materialises for a graph reaching millions of
/// nodes.
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
    // reallocating per level. Each node's depth is its parent's plus one.
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

/// Single-source shortest paths over forward rels (`weight` rel property when
/// `weighted`, else unit); unreachable nodes get `f64::INFINITY`.
pub fn sssp(g: &GraphSnapshot, source: u32, directed: bool, weighted: bool) -> Vec<f64> {
    let sp = g.dijkstra(source, fwd(directed), &[] as &[&str], None, |_from, rel| {
        if weighted {
            g.rel_prop(rel.pos, "weight").f64_or(1.0)
        } else {
            1.0
        }
    });
    (0..g.node_count())
        .map(|v| sp.distance(v).unwrap_or(f64::INFINITY))
        .collect()
}

/// Weakly connected components: each node's label is the smallest node id in
/// its component, found by flooding undirected (`Direction::Both`) rels. Nodes
/// are swept in ascending id order, so the first-reached node of a component is
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
/// uniformly. Forward rels are outgoing (directed) or both (undirected).
pub fn pagerank(g: &GraphSnapshot, directed: bool, damping: f64, iterations: u32) -> Vec<f64> {
    let n = g.node_count() as usize;
    if n == 0 {
        return Vec::new();
    }
    let nf = n as f64;
    let out = fwd(directed);
    // Pull formulation: each node sums its in-neighbours' shares, writing `next[v]`
    // disjointly so it parallelizes (the push `next[w] += ...` would race). In-rels
    // are incoming for a directed graph, both for undirected.
    let in_dir = if directed {
        Direction::Incoming
    } else {
        Direction::Both
    };
    let outdeg: Vec<u32> = (0..n as u32)
        .map(|v| g.neighbors(v, out).count() as u32)
        .collect();
    let mut pr = vec![1.0 / nf; n];
    let mut next = vec![0.0_f64; n];
    let workers = std::env::var("GA_THREADS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, |p| p.get()));
    const BATCH: u32 = 2048;
    for _ in 0..iterations {
        // Sinks (out-degree 0) redistribute their rank uniformly through `base`.
        let dangling: f64 = (0..n).filter(|&v| outdeg[v] == 0).map(|v| pr[v]).sum();
        let base = (1.0 - damping) / nf + damping * dangling / nf;
        let cursor = std::sync::atomic::AtomicU32::new(0);
        let nxt_ptr = next.as_mut_ptr() as usize;
        let (pr_ref, outdeg_ref): (&[f64], &[u32]) = (&pr, &outdeg);
        let cursor = &cursor;
        std::thread::scope(|scope| {
            for _ in 0..workers {
                scope.spawn(move || loop {
                    let start = cursor.fetch_add(BATCH, std::sync::atomic::Ordering::Relaxed);
                    if start as usize >= n {
                        break;
                    }
                    let end = (start as usize + BATCH as usize).min(n) as u32;
                    for v in start..end {
                        let mut pull = 0.0_f64;
                        for u in g.neighbors(v, in_dir) {
                            let d = outdeg_ref[u as usize];
                            if d > 0 {
                                pull += pr_ref[u as usize] / d as f64;
                            }
                        }
                        // SAFETY: the atomic cursor hands out disjoint node batches,
                        // so the `next` slots never alias; `next` outlives the scope.
                        unsafe {
                            *(nxt_ptr as *mut f64).add(v as usize) = base + damping * pull;
                        }
                    }
                });
            }
        });
        std::mem::swap(&mut pr, &mut next);
    }
    pr
}

/// Community detection by synchronous label propagation: `L0(v) = v`, then each
/// node adopts the most frequent label among its neighbours (incoming and
/// outgoing tallied separately for directed graphs, so a mutual rel counts the
/// label twice; each neighbour once for undirected), smallest label breaking
/// ties. A node with no neighbours keeps its label. Runs `iterations` rounds.
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
    // Each iteration is a synchronous map over independent nodes (read `cur`,
    // write `nxt`), so parallelize it: workers steal node batches via an atomic
    // cursor and write disjoint `nxt` slots, with the scope as the barrier before
    // the swap. Per-worker label buffers are reused across batches and iterations.
    let workers = std::env::var("GA_THREADS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, |p| p.get()));
    let mut bufs: Vec<Vec<u32>> = (0..workers).map(|_| Vec::new()).collect();
    const BATCH: u32 = 2048;
    for _ in 0..iterations {
        let cursor = std::sync::atomic::AtomicU32::new(0);
        let nxt_ptr = nxt.as_mut_ptr() as usize;
        let cur_ref: &[u32] = &cur;
        let cursor = &cursor;
        std::thread::scope(|scope| {
            for buf in bufs.iter_mut() {
                scope.spawn(move || loop {
                    let start = cursor.fetch_add(BATCH, std::sync::atomic::Ordering::Relaxed);
                    if start >= n {
                        break;
                    }
                    let end = (start + BATCH).min(n);
                    for v in start..end {
                        let label = cdlp_label(g, directed, cur_ref, v, buf);
                        // SAFETY: the atomic cursor hands out disjoint node batches,
                        // so the `nxt` slots never alias; `nxt` outlives the scope.
                        unsafe {
                            *(nxt_ptr as *mut u32).add(v as usize) = label;
                        }
                    }
                });
            }
        });
        std::mem::swap(&mut cur, &mut nxt);
    }
    cur
}

/// One synchronous CDLP update for node `v`: gather neighbour labels from `cur`
/// (in+out for directed, so a mutual rel counts the label twice; each neighbour
/// once for undirected) into the reused `buf`, then return the most frequent label
/// -- smallest on a tie, via a sort + run scan -- defaulting to `cur[v]` when `v`
/// has no neighbours.
fn cdlp_label(g: &GraphSnapshot, directed: bool, cur: &[u32], v: u32, buf: &mut Vec<u32>) -> u32 {
    buf.clear();
    if directed {
        buf.extend(g.neighbors(v, Direction::Outgoing).map(|u| cur[u as usize]));
        buf.extend(g.neighbors(v, Direction::Incoming).map(|u| cur[u as usize]));
    } else {
        buf.extend(g.neighbors(v, Direction::Both).map(|u| cur[u as usize]));
    }
    buf.sort_unstable();
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
    best_label
}

/// Local clustering coefficient: for each node `v` with undirected neighbour set
/// `N(v)` (each neighbour once, self excluded), `0` if `|N(v)| <= 1` else the
/// number of forward rels running between members of `N(v)` divided by
/// `|N(v)|*(|N(v)|-1)`. Forward rels are outgoing (directed) or both (undirected).
pub fn lcc(g: &GraphSnapshot, directed: bool) -> Vec<f64> {
    let n = g.node_count();
    let out = fwd(directed);

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

    // The per-node triangle count is independent across nodes, so split the
    // range across cores: each worker owns its membership bitset + neighbour
    // buffer, reads the shared adjacency, and writes a disjoint slice of `result`.
    let mut result = vec![0.0_f64; n as usize];
    let workers = std::env::var("GA_THREADS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or_else(|| std::thread::available_parallelism().map_or(1, |p| p.get()));
    // Dynamic work-stealing over small batches: workers pull the next [start,end)
    // range via an atomic cursor, so an uneven hub distribution can't strand one
    // worker with all the heavy nodes (static contiguous chunks could).
    let cursor = std::sync::atomic::AtomicU32::new(0);
    let rptr = result.as_mut_ptr() as usize;
    const BATCH: u32 = 2048;
    let (off, adj, cursor) = (&off, &adj, &cursor);
    std::thread::scope(|scope| {
        for _ in 0..workers {
            scope.spawn(move || {
                // Per-worker scratch, allocated once and reused across every batch
                // this worker steals (not per call -- that would allocate a bitset
                // per batch).
                let mut mark = vec![0u64; (n as usize + 63) / 64];
                let mut nbrs: Vec<u32> = Vec::new();
                loop {
                    let start = cursor.fetch_add(BATCH, std::sync::atomic::Ordering::Relaxed);
                    if start >= n {
                        break;
                    }
                    let end = (start + BATCH).min(n);
                    // SAFETY: atomic fetch_add hands out disjoint [start,end) batches,
                    // so the slices never alias; `result` outlives the scope.
                    let slice = unsafe {
                        std::slice::from_raw_parts_mut(
                            (rptr as *mut f64).add(start as usize),
                            (end - start) as usize,
                        )
                    };
                    lcc_count_range(g, off, adj, start, slice, &mut mark, &mut nbrs);
                }
            });
        }
    });
    result
}

/// Local clustering coefficient for nodes `start .. start + result.len()`,
/// written into `result`. Worker body for [`lcc`]: `mark` (the N(v) membership
/// bitset -- ~1/32 a u32 marker array, cache-resident) and `nbrs` are caller-owned
/// scratch reused across every batch a worker steals; `off`/`adj` are the shared
/// sorted forward-adjacency.
fn lcc_count_range(
    g: &GraphSnapshot,
    off: &[u32],
    adj: &[u32],
    start: u32,
    result: &mut [f64],
    mark: &mut [u64],
    nbrs: &mut Vec<u32>,
) {
    for (i, slot) in result.iter_mut().enumerate() {
        let v = start + i as u32;
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
            // The gallop branch needs `nbrs` sorted; sort once iff some neighbour is
            // high-degree (the scan branch is order-independent, so skip otherwise).
            if nbrs
                .iter()
                .any(|&u| (off[u as usize + 1] - off[u as usize]) as usize > k)
            {
                nbrs.sort_unstable();
            }
            let mut rels = 0u64;
            for &u in nbrs.iter() {
                let uo = &adj[off[u as usize] as usize..off[u as usize + 1] as usize];
                if uo.len() <= k {
                    // Short out-list: scan it, testing N(v) membership by bit.
                    for &w in uo {
                        if w != u && mark[(w >> 6) as usize] >> (w & 63) & 1 != 0 {
                            rels += 1;
                        }
                    }
                } else {
                    // High-degree u: gallop-merge the sorted neighbour set against
                    // u's out-list -- one monotonic cursor through `uo` instead of k
                    // independent (cache-cold) binary searches.
                    let mut cursor = 0;
                    for &w in nbrs.iter() {
                        if w == u {
                            continue;
                        }
                        let (found, pos) = gallop(uo, cursor, w);
                        cursor = pos;
                        if found {
                            rels += 1;
                        }
                    }
                }
            }
            *slot = rels as f64 / (k as f64 * (k as f64 - 1.0));
        }
        // Reset N(v)'s bits for the next node (including the k < 2 case).
        for &u in nbrs.iter() {
            mark[(u >> 6) as usize] &= !(1u64 << (u & 63));
        }
    }
}

/// Galloping (exponential) search for `target` in `uo[from..]`: returns whether it
/// is present and the index where it is or would be inserted (always `>= from`).
/// Called with a cursor that only advances, so successive lookups walk `uo`
/// monotonically -- the cache-locality win over independent binary searches.
fn gallop(uo: &[u32], from: usize, target: u32) -> (bool, usize) {
    let n = uo.len();
    if from >= n {
        return (false, n);
    }
    let mut bound = 1;
    while from + bound < n && uo[from + bound] < target {
        bound *= 2;
    }
    let lo = from + bound / 2;
    let hi = (from + bound + 1).min(n);
    match uo[lo..hi].binary_search(&target) {
        Ok(i) => (true, lo + i),
        Err(i) => (false, lo + i),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustychickpeas_core::GraphBuilder;

    /// Build a graph of `n` nodes (labelled `V`) wired by the given `e` rels.
    fn build(n: u32, rels: &[(u32, u32)]) -> GraphSnapshot {
        let mut b = GraphBuilder::new(Some(n as usize), Some(rels.len()));
        for i in 0..n {
            b.add_node(Some(i), &["V"]).unwrap();
        }
        for &(u, v) in rels {
            b.add_relationship(u, v, "e").unwrap();
        }
        b.finalize(None)
    }

    /// As [`build`], but each rel carries a `weight` f64 property for SSSP.
    fn build_weighted(n: u32, rels: &[(u32, u32, f64)]) -> GraphSnapshot {
        let mut b = GraphBuilder::new(Some(n as usize), Some(rels.len()));
        for i in 0..n {
            b.add_node(Some(i), &["V"]).unwrap();
        }
        for &(u, v, w) in rels {
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
        // Undirected triangle: every node collapses to the smallest label (0).
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

    #[test]
    fn lcc_gallop_branch_on_high_degree_neighbour() {
        // N(0) = {1,2} (k=2); node 1 has out-degree 3 (> k), so counting rels among
        // N(0) takes the gallop branch. The single inside-rel is 1->2, giving
        // LCC(0) = 1 / (2*1) = 0.5.
        let g = build(5, &[(0, 1), (0, 2), (1, 2), (1, 3), (1, 4)]);
        let coeffs = lcc(&g, true);
        assert!((coeffs[0] - 0.5).abs() < 1e-9, "{}", coeffs[0]);
    }
}

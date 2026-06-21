"""The six LDBC Graphalytics algorithms over a rustychickpeas GraphSnapshot,
hand-ported from src/graphalytics/mod.rs. Outputs are node-indexed lists (index ==
dense node id). ``directed`` selects the forward direction: Outgoing (directed) or
Both (undirected, whose rels are stored once).

Adjacency is read with the *untyped* ``neighbor_ids(v, dir)`` (the graph has only
``e`` rels), which — unlike the typed form — does not dedup, matching the Rust
``g.neighbors`` multiset; ``degree(v, dir)`` is the O(1) CSR count.
"""

import heapq
from collections import Counter

from rustychickpeas import Direction

BFS_UNREACHABLE = 9223372036854775807  # i64::MAX, per the spec


def _fwd(directed):
    return Direction.Outgoing if directed else Direction.Both


def _in_dir(directed):
    return Direction.Incoming if directed else Direction.Both


def bfs(g, source, n, directed):
    """Breadth-first hop depth from `source` over forward rels; unreachable nodes
    get i64::MAX. Uses the native bfs_distances kernel."""
    dist = [BFS_UNREACHABLE] * n
    if source < 0 or source >= n:
        return dist
    for node, hops in g.bfs_distances(source, _fwd(directed)).items():
        dist[node] = hops
    return dist


def sssp(g, source, n, directed, weighted=True):
    """Single-source shortest paths over forward rels (`weight` rel prop when
    weighted, else unit); unreachable nodes get inf. Pure-Python binary-heap
    Dijkstra reading rel weights via the native rels_with_props."""
    dist = [float("inf")] * n
    if source < 0 or source >= n:
        return dist
    direction = _fwd(directed)
    dist[source] = 0.0
    pq = [(0.0, source)]
    while pq:
        d, u = heapq.heappop(pq)
        if d > dist[u]:
            continue
        nbrs, cols = g.rels_with_props(u, direction, "e", ["weight"])
        weights = cols[0]
        for i, v in enumerate(nbrs):
            w = weights[i] if weighted and weights[i] is not None else 1.0
            nd = d + w
            if nd < dist[v]:
                dist[v] = nd
                heapq.heappush(pq, (nd, v))
    return dist


def wcc(g, n):
    """Weakly connected components: each node's label is the smallest node id in its
    component (sweep ascending, flood Direction.Both)."""
    UNSEEN = -1
    comp = [UNSEEN] * n
    nbr = g.neighbor_ids
    for s in range(n):
        if comp[s] != UNSEEN:
            continue
        comp[s] = s
        stack = [s]
        while stack:
            v = stack.pop()
            for u in nbr(v, Direction.Both):
                if comp[u] == UNSEEN:
                    comp[u] = s
                    stack.append(u)
    return comp


def pagerank(g, n, directed, damping, iterations):
    """PageRank after `iterations` synchronous pull updates with `damping`; sinks
    (out-degree 0) redistribute their rank uniformly."""
    if n == 0:
        return []
    nf = float(n)
    out_dir, in_dir = _fwd(directed), _in_dir(directed)
    outdeg = [g.degree(v, out_dir) for v in range(n)]
    in_nbrs = [g.neighbor_ids(v, in_dir) for v in range(n)]  # cache adjacency once
    sinks = [v for v in range(n) if outdeg[v] == 0]
    pr = [1.0 / nf] * n
    for _ in range(iterations):
        dangling = 0.0
        for v in sinks:
            dangling += pr[v]
        base = (1.0 - damping) / nf + damping * dangling / nf
        nxt = [base] * n
        for v in range(n):
            pull = 0.0
            for u in in_nbrs[v]:
                du = outdeg[u]
                if du:
                    pull += pr[u] / du
            if pull:
                nxt[v] = base + damping * pull
        pr = nxt
    return pr


def cdlp(g, n, directed, iterations, seed):
    """Community detection by synchronous label propagation seeded with `seed[node]`
    (use vertex ids to match a vertex-keyed reference). Each node adopts the most
    frequent neighbour label — smallest on a tie — counting in+out separately for a
    directed graph (a mutual rel counts twice); a node with no neighbours keeps its
    label."""
    cur = list(seed)
    if directed:
        out_n = [g.neighbor_ids(v, Direction.Outgoing) for v in range(n)]
        in_n = [g.neighbor_ids(v, Direction.Incoming) for v in range(n)]
    else:
        both_n = [g.neighbor_ids(v, Direction.Both) for v in range(n)]
    for _ in range(iterations):
        nxt = [0] * n
        for v in range(n):
            if directed:
                labs = [cur[u] for u in out_n[v]]
                labs += [cur[u] for u in in_n[v]]
            else:
                labs = [cur[u] for u in both_n[v]]
            if not labs:
                nxt[v] = cur[v]
                continue
            counts = Counter(labs)
            # most frequent, smallest label breaking ties.
            nxt[v] = min(counts.items(), key=lambda kv: (-kv[1], kv[0]))[0]
        cur = nxt
    return cur


def lcc(g, n, directed):
    """Local clustering coefficient: for each node v with undirected neighbour set
    N(v) (each once, self excluded), 0 if |N(v)|<=1 else the number of forward rels
    between members of N(v) over |N(v)|*(|N(v)|-1)."""
    out_dir = _fwd(directed)
    fwd_adj = [set(g.neighbor_ids(v, out_dir)) for v in range(n)]
    result = [0.0] * n
    for v in range(n):
        nbrs = set(g.neighbor_ids(v, Direction.Both))
        nbrs.discard(v)
        k = len(nbrs)
        if k < 2:
            continue
        rels = 0
        for u in nbrs:
            for w in fwd_adj[u]:
                if w != u and w in nbrs:
                    rels += 1
        result[v] = rels / (k * (k - 1.0))
    return result

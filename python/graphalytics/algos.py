"""The six LDBC Graphalytics algorithms over a rustychickpeas GraphSnapshot.

Each prefers the native core kernel (`g.pagerank`/`wcc`/`cdlp`/`lcc`/`sssp`,
`bfs_distances`) when the binding exposes it — these run in Rust with the GIL
released — and falls back to the pure-Python reference implementation otherwise (an
older wheel). Outputs are node-indexed lists (index == dense node id). ``directed``
selects the forward direction: Outgoing (directed) or Both (undirected, rels stored
once).

The pure-Python fallbacks read adjacency with the *untyped* ``neighbor_ids(v, dir)``
(the graph has only ``e`` rels), which — unlike the typed form — does not dedup,
matching the Rust ``g.neighbors`` multiset; ``degree(v, dir)`` is the O(1) CSR count.
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
    weighted, else unit); unreachable nodes get inf. Native `sssp` kernel when
    present, else a pure-Python binary-heap Dijkstra."""
    fn = getattr(g, "sssp", None)
    if fn is not None:
        return fn(source, directed, "weight" if weighted else None)
    return _sssp_py(g, source, n, directed, weighted)


def wcc(g, n):
    """Weakly connected components: each node's label is the smallest node id in its
    component. Native `wcc` kernel when present, else a pure-Python flood."""
    fn = getattr(g, "wcc", None)
    if fn is not None:
        return fn()
    return _wcc_py(g, n)


def pagerank(g, n, directed, damping, iterations):
    """PageRank after `iterations` synchronous pull updates with `damping` (sinks
    redistribute uniformly). Native `pagerank` kernel when present, else pure-Python."""
    fn = getattr(g, "pagerank", None)
    if fn is not None:
        return fn(directed, damping, iterations)
    return _pagerank_py(g, n, directed, damping, iterations)


def cdlp(g, n, directed, iterations, seed):
    """Community detection by synchronous label propagation seeded with `seed[node]`
    (vertex ids match a vertex-keyed reference). Native `cdlp` kernel when present,
    else pure-Python."""
    fn = getattr(g, "cdlp", None)
    if fn is not None:
        return fn(directed, iterations, list(seed))
    return _cdlp_py(g, n, directed, iterations, seed)


def lcc(g, n, directed):
    """Local clustering coefficient per node. Native `lcc` kernel when present, else
    a pure-Python triangle count."""
    fn = getattr(g, "lcc", None)
    if fn is not None:
        return fn(directed)
    return _lcc_py(g, n, directed)


# ---- pure-Python reference fallbacks ---------------------------------------


def _sssp_py(g, source, n, directed, weighted):
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


def _wcc_py(g, n):
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


def _pagerank_py(g, n, directed, damping, iterations):
    if n == 0:
        return []
    nf = float(n)
    out_dir, in_dir = _fwd(directed), _in_dir(directed)
    outdeg = [g.degree(v, out_dir) for v in range(n)]
    in_nbrs = [g.neighbor_ids(v, in_dir) for v in range(n)]
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


def _cdlp_py(g, n, directed, iterations, seed):
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
            nxt[v] = min(counts.items(), key=lambda kv: (-kv[1], kv[0]))[0]
        cur = nxt
    return cur


def _lcc_py(g, n, directed):
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

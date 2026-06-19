"""BI Q15 — weighted interaction path.

Weighted shortest path over the knows graph where each edge weight is 1/(w+1) and
w sums reply interactions between the two people whose thread root-post forum was
created in [start_day, end_day] (1.0 if a Post is replied to, else 0.5). Returns the
path cost, or -1 if unreachable.

The interaction weights are built once over comments (memoized thread roots + per-
root forum), then a heap Dijkstra runs over the full knows graph (every edge
traversable, weight 1/(w+1)).
"""

import heapq

from rustychickpeas import Direction


_NO_NEIGHBOR = 0xFFFFFFFF  # neighbor_via sentinel for "no such neighbor"


def _build_weights(g, start_day, end_day):
    posts = set(g.nodes_with_label("Post"))
    roots = memoryview(g.roots_via("replyOf", Direction.Outgoing))  # message -> thread root
    creators = memoryview(g.neighbor_via("hasCreator", Direction.Incoming))  # message -> creator
    forum_fday = {}   # thread root -> its forum's fday (None if no forum)

    def fday_of_root(root):
        if root in forum_fday:
            return forum_fday[root]
        forum = g.first_neighbor(root, Direction.Incoming, "containerOf")
        v = (g.get_property(forum, "fday") or 0) if forum is not None else None
        forum_fday[root] = v
        return v

    w = {}
    for c in g.nodes_with_label("Comment"):
        parent = g.first_neighbor(c, Direction.Outgoing, "replyOf")
        if parent is None:
            continue
        cc = creators[c]
        pc = creators[parent]
        if cc == _NO_NEIGHBOR or pc == _NO_NEIGHBOR or cc == pc:
            continue
        fday = fday_of_root(roots[c])
        if fday is not None and start_day <= fday <= end_day:
            contrib = 1.0 if parent in posts else 0.5
            key = (cc, pc) if cc < pc else (pc, cc)
            w[key] = w.get(key, 0.0) + contrib
    return w


def q15_weighted_path(g, p1_plid, p2_plid, start_day, end_day):
    src = g.node_with_label_property("Person", "id", p1_plid)
    tgt = g.node_with_label_property("Person", "id", p2_plid)
    if src is None or tgt is None:
        return -1.0

    w = _build_weights(g, start_day, end_day)
    dist = {src: 0.0}
    pq = [(0.0, src)]
    while pq:
        d, u = heapq.heappop(pq)
        if u == tgt:
            return d
        if d > dist.get(u, float("inf")):
            continue
        for v in g.neighbor_ids(u, Direction.Outgoing, ["knows"]):
            weight = 1.0 / (w.get((u, v) if u < v else (v, u), 0.0) + 1.0)
            nd = d + weight
            if nd < dist.get(v, float("inf")):
                dist[v] = nd
                heapq.heappush(pq, (nd, v))
    return -1.0

"""BI Q19 — interaction path between cities.

For people in city1 and city2, find the shortest weighted path on the knows graph
where each edge weight is 1/(reply interactions between the two people); return the
20 city1-city2 pairs with the smallest path weight. Reports ``(p1_id, p2_id, dist)``.

The search graph is the knows edges whose endpoints actually replied to each other
(weight 1/n); a plain heap Dijkstra over that small projected graph per city1 person
reaches every city2 target in one sweep.
"""

import heapq

from rustychickpeas import Direction


def build_interaction_map(g):
    """Map each undirected person pair to their reply-interaction count: for each
    comment, its creator replied to the parent message's creator. This is the
    one-mode projection of ``replyOf`` through each message's creator — the native
    ``fold_via`` kernel folds it in parallel (replyOf edges originate only from
    Comments, so folding every node matches the per-Comment scan)."""
    creators = g.neighbor_via("hasCreator", Direction.Incoming)
    return g.fold_via("replyOf", Direction.Outgoing, creators)


def _dijkstra(g, src, interaction):
    """Shortest weighted distances from src over knows edges with edge weight
    1/interaction (edges with no interaction are not traversable)."""
    dist = {src: 0.0}
    pq = [(0.0, src)]
    while pq:
        d, u = heapq.heappop(pq)
        if d > dist.get(u, float("inf")):
            continue
        for v in g.neighbor_ids(u, Direction.Outgoing, ["knows"]):
            n = interaction.get((u, v) if u < v else (v, u), 0)
            if n <= 0:
                continue
            nd = d + 1.0 / n
            if nd < dist.get(v, float("inf")):
                dist[v] = nd
                heapq.heappush(pq, (nd, v))
    return dist


def q19_interaction_path(g, city1_id, city2_id, interaction):
    city1 = g.node_with_label_property("City", "id", city1_id)
    city2 = g.node_with_label_property("City", "id", city2_id)
    if city1 is None or city2 is None:
        return []

    c2 = set(g.neighbor_ids(city2, Direction.Incoming, ["isLocatedIn"]))
    results = []
    for p1 in g.neighbor_ids(city1, Direction.Incoming, ["isLocatedIn"]):
        dist = _dijkstra(g, p1, interaction)
        p1id = g.get_property(p1, "id")
        for p2 in c2:
            d = dist.get(p2)
            if d is not None:
                results.append((p1id, g.get_property(p2, "id"), d))

    results.sort(key=lambda r: (r[2], r[0], r[1]))
    return results[:20]

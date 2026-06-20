"""IC13 — unweighted shortest-path length between two persons over ``knows``
(-1 if unreachable). Hop distance from a bounded-free BFS.
"""

from rustychickpeas import Direction


def ic13_shortest_path(g, p1, p2):
    if p1 == p2:
        return 0
    dist = g.bfs_distances(p1, Direction.Outgoing, rel_types=["knows"])
    return dist.get(p2, -1)

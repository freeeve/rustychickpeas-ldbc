"""BI Q20 — recruitment.

From each employee of a company, the shortest weighted path on the knows graph to
a target person, where edge weight is the closeness of the two people's university
cohorts (min |classYear difference| + 1); return the 20 employees with the smallest
path weight. Reports ``(person_id, dist)``.

A study-cohort weight map (built once over knows pairs who studied together) drives
a target-early-exit heap Dijkstra per employee.
"""

import heapq

from rustychickpeas import Direction


def build_studyat(g):
    """Map each person to their (university, classYear) study records."""
    m = {}
    for p in g.nodes_with_label("Person"):
        recs = [
            (rel.end_node().id(), rel.get_property("cy") or 0)
            for rel in g.relationships(p, Direction.Outgoing, ["studyAt"])
        ]
        if recs:
            m[p] = recs
    return m


def build_study_weight_map(g, studyat):
    """For knowing persons who studied at a common university, min |classYear
    difference| + 1 (smaller = closer cohort), keyed by the (min, max) id pair."""
    wm = {}
    for a, sa in studyat.items():
        for b in g.neighbor_ids(a, Direction.Outgoing, ["knows"]):
            if b <= a:
                continue
            sb = studyat.get(b)
            if sb is None:
                continue
            best = None
            for ua, ya in sa:
                for ub, yb in sb:
                    if ua == ub:
                        diff = abs(ya - yb)
                        best = diff if best is None else min(best, diff)
            if best is not None:
                wm[(a, b)] = float(best + 1)
    return wm


def _dijkstra(g, src, weight_map, target):
    """Shortest weighted distance from src to target over knows edges weighted by
    weight_map (untraversable where absent); target early-exit. None if unreachable."""
    dist = {src: 0.0}
    pq = [(0.0, src)]
    while pq:
        d, u = heapq.heappop(pq)
        if u == target:
            return d
        if d > dist.get(u, float("inf")):
            continue
        for v in g.neighbor_ids(u, Direction.Outgoing, ["knows"]):
            w = weight_map.get((u, v) if u < v else (v, u))
            if w is None:
                continue
            nd = d + w
            if nd < dist.get(v, float("inf")):
                dist[v] = nd
                heapq.heappush(pq, (nd, v))
    return dist.get(target)


def q20_recruitment(g, company_name, person2_plid, weight_map):
    company = g.node_with_label_property("Company", "name", company_name)
    person2 = g.node_with_label_property("Person", "id", person2_plid)
    if company is None or person2 is None:
        return []

    results = []
    for p1 in g.neighbor_ids(company, Direction.Incoming, ["workAt"]):
        if p1 == person2:
            continue
        d = _dijkstra(g, p1, weight_map, person2)
        if d is not None:
            results.append((g.get_property(p1, "id"), d))

    results.sort(key=lambda r: (r[1], r[0]))
    return results[:20]

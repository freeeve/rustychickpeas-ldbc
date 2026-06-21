"""BI Q11 — friend triangles in a country within a date window.

Count triangles of persons located in ``country`` where all three knows rels
were created within [start_day, end_day]. Returns the triangle count.

Optimization (task 177): the date-filtered knows adjacency is built via the bulk
``rels_with_props`` accessor — aligned ``(neighbor_id, kd)`` arrays read from the
rel column — instead of ``relationships()`` which constructs a ``Node`` per rel just
to read its id and a ``get_property`` per rel for ``kd``.
"""

from rustychickpeas import Direction


def q11_friend_triangles(g, country_name: str, start_day: int, end_day: int) -> int:
    country = g.node_with_label_property("Country", "name", country_name)
    if country is None:
        return 0

    in_country = set()
    for city in g.neighbor_ids(country, Direction.Incoming, ["isPartOf"]):
        in_country.update(g.neighbor_ids(city, Direction.Incoming, ["isLocatedIn"]))

    adj = {}  # person -> set of in-country knows neighbors with rel kd in window
    for a in in_country:
        neighbors, cols = g.rels_with_props(a, Direction.Outgoing, "knows", ["kd"])
        kds = cols[0]
        nbrs = set()
        for i, nbr in enumerate(neighbors):
            if nbr in in_country:
                kd = kds[i]
                if kd is not None and start_day <= kd <= end_day:
                    nbrs.add(nbr)
        if nbrs:
            adj[a] = nbrs

    count = 0
    for a, nbrs_a in adj.items():
        for b in nbrs_a:
            if b <= a:
                continue
            nbrs_b = adj.get(b)
            if nbrs_b:
                for c in nbrs_b:
                    if c > b and c in nbrs_a:
                        count += 1
    return count

"""BI Q11 — friend triangles in a country within a date window.

Count triangles of persons located in ``country`` where all three knows rels
were created within [start_day, end_day]. Returns the triangle count.

Existing primitives: build a date-filtered knows adjacency among the country's
persons (reading each knows rel's ``kd`` creation-day property), then count
triangles a<b<c with all three rels present.
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
        nbrs = set()
        for rel in g.relationships(a, Direction.Outgoing, ["knows"]):
            nbr = rel.end_node().id()
            if nbr in in_country:
                kd = rel.get_property("kd")
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

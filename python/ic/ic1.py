"""IC1 — friends within 3 ``knows`` hops whose first name matches ``first_name``,
ordered by (distance, lastName, id). Returns (friend, distance, lastName), top 20.
"""

from rustychickpeas import Direction


def ic1_friends_by_name(g, person, first_name):
    dist = g.bfs_distances(person, Direction.Outgoing, rel_types=["knows"], max_depth=3)
    rows = [
        (p, d, g.prop_str(p, "lname") or "")
        for p, d in dist.items()
        if d >= 1 and g.prop_str(p, "fname") == first_name
    ]
    rows.sort(key=lambda r: (r[1], r[2], g.get_property(r[0], "id")))
    return rows[:20]

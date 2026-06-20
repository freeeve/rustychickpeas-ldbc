"""IC3 — friends and friends-of-friends (1..=2 ``knows`` hops, excluding self and
anyone whose home Country is X or Y) who created messages located in BOTH Country X
and Country Y within ``[start_day, start_day + duration_days)``. Returns
(person, x_count, y_count), (x+y desc, id asc), top 20.
"""

from rustychickpeas import Direction

from ._cols import i64_reader


def ic3_friends_two_countries(g, person, country_x, country_y, start_day, duration_days):
    end_day = start_day + duration_days
    cx = g.node_with_label_property("Country", "name", country_x)
    cy = g.node_with_label_property("Country", "name", country_y)
    if cx is None or cy is None:
        return []
    day = i64_reader(g, "day")
    rows = []
    for p in g.neighborhood(person, Direction.Outgoing, "knows", 2, min_hops=1):
        home = g.follow(p, [(Direction.Outgoing, "isLocatedIn"),
                            (Direction.Outgoing, "isPartOf")])
        if home == cx or home == cy:
            continue
        xc = yc = 0
        for msg in g.neighbor_ids(p, Direction.Outgoing, ["hasCreator"]):
            d = day(msg)
            if d < start_day or d >= end_day:
                continue
            c = g.first_neighbor(msg, Direction.Outgoing, "msgCountry")
            if c == cx:
                xc += 1
            elif c == cy:
                yc += 1
        if xc > 0 and yc > 0:
            rows.append((p, xc, yc))
    rows.sort(key=lambda r: (-(r[1] + r[2]), g.get_property(r[0], "id")))
    return rows[:20]

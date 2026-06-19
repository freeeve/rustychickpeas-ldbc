"""BI Q13 — zombies in a country.

A zombie is a low-activity person in ``country``: created before end_day with
fewer than one message per month of life (up to end_day). Score each zombie by
the share of likes on their messages that come from other zombies; top 100 by
that ratio (person id ascending on ties). Reports
``(person_id, zombie_like_count, total_like_count)``.

Existing primitives: a country-membership scan with the person ``pday``/``pym``
creation properties, then a like-source tally over each zombie's messages.
"""

from rustychickpeas import Direction


def q13_zombies(g, country_name: str, end_day: int, end_ym: int):
    country = g.node_with_label_property("Country", "name", country_name)
    if country is None:
        return []

    zombies = set()
    for city in g.neighbor_ids(country, Direction.Incoming, ["isPartOf"]):
        for p in g.neighbor_ids(city, Direction.Incoming, ["isLocatedIn"]):
            if (g.get_property(p, "pday") or 0) >= end_day:
                continue
            mcount = sum(
                1
                for m in g.neighbor_ids(p, Direction.Outgoing, ["hasCreator"])
                if (g.get_property(m, "day") or 0) < end_day
            )
            months = end_ym - (g.get_property(p, "pym") or 0) + 1
            if months > 0 and mcount < months:
                zombies.add(p)

    rows = []
    for z in zombies:
        zlc = tlc = 0
        for m in g.neighbor_ids(z, Direction.Outgoing, ["hasCreator"]):
            for liker in g.neighbor_ids(m, Direction.Incoming, ["likes"]):
                if (g.get_property(liker, "pday") or 0) < end_day:
                    tlc += 1
                if liker in zombies:
                    zlc += 1
        rows.append((g.get_property(z, "id"), zlc, tlc))

    rows.sort(key=lambda r: (-(r[1] / r[2] if r[2] else 0.0), r[0]))
    return rows[:100]

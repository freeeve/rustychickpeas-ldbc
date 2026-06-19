"""BI Q14 — international dialog.

For each city of country1, find the knows-pair (a person in that city, a person in
country2) with the highest interaction score: +4 if p1 replied to p2, +1 if p2
replied to p1, +10 if p1 likes p2's message, +1 if p2 likes p1's. Ties on a pair
prefer the lower p1 id then lower p2 id. Top 100 by score, then p1 id, then p2 id.
Reports ``(p1_id, p2_id, city_name, score)``.

Existing primitives: per-person "replied-to" and "liked-creator" interaction sets,
precomputed for country2, then a best-pair scan over each city's residents.
"""

from rustychickpeas import Direction


def q14_international_dialog(g, c1_name: str, c2_name: str):
    country1 = g.node_with_label_property("Country", "name", c1_name)
    country2 = g.node_with_label_property("Country", "name", c2_name)
    if country1 is None or country2 is None:
        return []

    def commented_on(p):
        """Creators of the messages p's messages replied to (p replied to them)."""
        s = set()
        for msg in g.neighbor_ids(p, Direction.Outgoing, ["hasCreator"]):
            for parent in g.neighbor_ids(msg, Direction.Outgoing, ["replyOf"]):
                cr = g.first_neighbor(parent, Direction.Incoming, "hasCreator")
                if cr is not None:
                    s.add(cr)
        return s

    def liked_creators(p):
        """Creators of the messages p likes."""
        s = set()
        for msg in g.neighbor_ids(p, Direction.Outgoing, ["likes"]):
            cr = g.first_neighbor(msg, Direction.Incoming, "hasCreator")
            if cr is not None:
                s.add(cr)
        return s

    in_c2, co_c2, lc_c2 = set(), {}, {}
    for city in g.neighbor_ids(country2, Direction.Incoming, ["isPartOf"]):
        for p in g.neighbor_ids(city, Direction.Incoming, ["isLocatedIn"]):
            if p not in in_c2:
                in_c2.add(p)
                co_c2[p] = commented_on(p)
                lc_c2[p] = liked_creators(p)

    def plid(n):
        return g.get_property(n, "id")

    rows = []
    for city in g.neighbor_ids(country1, Direction.Incoming, ["isPartOf"]):
        city_name = g.prop_str(city, "name") or ""
        best = None  # (score, -p1plid, -p2plid, p1plid, p2plid)
        for p1 in g.neighbor_ids(city, Direction.Incoming, ["isLocatedIn"]):
            p1_co = commented_on(p1)
            p1_lc = liked_creators(p1)
            for p2 in g.neighbor_ids(p1, Direction.Outgoing, ["knows"]):
                if p2 not in in_c2:
                    continue
                score = 0
                if p2 in p1_co:
                    score += 4
                if p1 in co_c2[p2]:
                    score += 1
                if p2 in p1_lc:
                    score += 10
                if p1 in lc_c2[p2]:
                    score += 1
                pa, pb = plid(p1), plid(p2)
                cand = (score, -pa, -pb, pa, pb)
                if best is None or cand[:3] > best[:3]:
                    best = cand
        if best is not None:
            rows.append((best[3], best[4], city_name, best[0]))

    rows.sort(key=lambda r: (-r[3], r[0], r[1]))
    return rows[:100]

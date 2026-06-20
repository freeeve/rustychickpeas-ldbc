"""IC10 — friend recommendation. Friends-of-friends (exactly 2 ``knows`` hops) born
in [21st of ``month`` .. 22nd of the next month], scored by (# their Posts tagged
with a seed interest) - (# not). Top 10 by (score desc, id asc). Returns (foaf, score).
"""

from rustychickpeas import Direction

from ._cols import i64_reader


def ic10_friend_recommend(g, person, month):
    next_month = month % 12 + 1
    interests = set(g.neighbor_ids(person, Direction.Outgoing, ["hasInterest"]))
    posts = set(g.nodes_with_label("Post"))
    bmon = i64_reader(g, "bmon")
    bdom = i64_reader(g, "bdom")
    rows = []
    for foaf in g.neighborhood(person, Direction.Outgoing, "knows", 2, min_hops=2):
        bm, bd = bmon(foaf), bdom(foaf)
        if not ((bm == month and bd >= 21) or (bm == next_month and bd < 22)):
            continue
        common = uncommon = 0
        for msg in g.neighbor_ids(foaf, Direction.Outgoing, ["hasCreator"]):
            if msg not in posts:
                continue
            if any(t in interests for t in g.neighbor_ids(msg, Direction.Outgoing, ["hasTag"])):
                common += 1
            else:
                uncommon += 1
        rows.append((foaf, common - uncommon))
    rows.sort(key=lambda r: (-r[1], g.get_property(r[0], "id")))
    return rows[:10]

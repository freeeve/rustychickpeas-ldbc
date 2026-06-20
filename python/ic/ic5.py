"""IC5 — Forums the seed's friends/FoF (1..=2 ``knows`` hops, excluding self) joined
after ``min_day``, ranked by Posts in each Forum created by those post-``min_day``
members. Returns (forum, post_count), (count desc, id asc), top 20.
"""

from rustychickpeas import Direction


def ic5_new_groups(g, person, min_day):
    forum_counts = {}
    for p in g.neighborhood(person, Direction.Outgoing, "knows", 2, min_hops=1):
        # Forums p joined after min_day (hasMember is Forum->Person, so the forum is
        # the source of each incoming hasMember; hd is its join day).
        qforums = set()
        for rel in g.relationships(p, Direction.Incoming, ["hasMember"]):
            hd = rel.get_property("hd")
            if hd is not None and hd > min_day:
                qforums.add(rel.start_node().id())
        if not qforums:
            continue
        for post in g.neighbor_ids(p, Direction.Outgoing, ["hasCreator"]):
            forum = g.first_neighbor(post, Direction.Incoming, "containerOf")
            if forum is not None and forum in qforums:
                forum_counts[forum] = forum_counts.get(forum, 0) + 1
    rows = list(forum_counts.items())
    rows.sort(key=lambda r: (-r[1], g.get_property(r[0], "id")))
    return rows[:20]

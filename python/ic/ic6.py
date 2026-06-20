"""IC6 — tags co-occurring with a given tag on the Posts of the seed's friends and
friends-of-friends (1..=2 ``knows`` hops): among those Posts that carry ``tag_name``,
count the other tags. Top 10 by (count desc, tag name asc). Returns (tag_id, count).
"""

from rustychickpeas import Direction


def ic6_tag_cooccurrence(g, person, tag_name):
    target = g.node_with_label_property("Tag", "name", tag_name)
    if target is None:
        return []
    posts = set(g.nodes_with_label("Post"))
    counts = {}
    for p in g.neighborhood(person, Direction.Outgoing, "knows", 2, min_hops=1):
        for post in g.neighbor_ids(p, Direction.Outgoing, ["hasCreator"]):
            if post not in posts:
                continue
            tags = set(g.neighbor_ids(post, Direction.Outgoing, ["hasTag"]))
            if target not in tags:
                continue
            for t in tags:
                if t != target:
                    counts[t] = counts.get(t, 0) + 1
    rows = list(counts.items())
    rows.sort(key=lambda r: (-r[1], g.prop_str(r[0], "name") or ""))
    return rows[:10]

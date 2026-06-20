"""IC12 — the seed's direct friends who replied (Comment -> replyOf -> Post) to Posts
tagged under ``class_name`` or a transitive subclass. Returns (friend, reply_count,
tag_names), (count desc, id asc), top 20.
"""

from rustychickpeas import Direction


def ic12_expert_search(g, person, class_name):
    root_class = g.node_with_label_property("TagClass", "name", class_name)
    if root_class is None:
        return []
    # The class plus all descendants (children point at the parent via isSubclassOf).
    class_set = set(g.bfs_distances(root_class, Direction.Incoming,
                                    rel_types=["isSubclassOf"]).keys())
    posts = set(g.nodes_with_label("Post"))
    qual = {}  # tag -> whether it has a type in class_set (memoized)

    def qual_tag(t):
        v = qual.get(t)
        if v is None:
            v = any(c in class_set for c in g.neighbor_ids(t, Direction.Outgoing, ["hasType"]))
            qual[t] = v
        return v

    rows = []
    for friend in g.neighbor_ids(person, Direction.Outgoing, ["knows"]):
        count = 0
        tag_ids = set()
        for c in g.neighbor_ids(friend, Direction.Outgoing, ["hasCreator"]):
            for parent in g.neighbor_ids(c, Direction.Outgoing, ["replyOf"]):
                if parent not in posts:
                    continue
                matched = False
                for t in g.neighbor_ids(parent, Direction.Outgoing, ["hasTag"]):
                    if qual_tag(t):
                        matched = True
                        tag_ids.add(t)
                if matched:
                    count += 1
        if count > 0:
            rows.append((friend, count, sorted((g.prop_str(t, "name") or "") for t in tag_ids)))
    rows.sort(key=lambda r: (-r[1], g.get_property(r[0], "id")))
    return rows[:20]

"""BI Q18 — friend recommendation.

For people interested in a tag, count the mutual friends shared with another
person also interested in the tag but not directly known; top 20 ordered pairs by
mutual-friend count (then p1 id, then p2 id ascending). Reports
``(p1_id, p2_id, mutual_count)``.

Existing primitives: for each interested p1 and each friend m, every interested
p2 known by m (distinct from p1 and not directly known by p1) shares m as a mutual.
"""

from rustychickpeas import Direction


def q18_friend_recommendation(g, tag_name: str):
    tag = g.node_with_label_property("Tag", "name", tag_name)
    if tag is None:
        return []

    interested = set(g.neighbor_ids(tag, Direction.Incoming, ["hasInterest"]))
    mutual = {}  # (p1, p2) -> set of mutual friends
    for p1 in interested:
        p1_knows = set(g.neighbor_ids(p1, Direction.Outgoing, ["knows"]))
        for m in p1_knows:
            for p2 in g.neighbor_ids(m, Direction.Outgoing, ["knows"]):
                if p2 != p1 and p2 in interested and p2 not in p1_knows:
                    mutual.setdefault((p1, p2), set()).add(m)

    rows = [
        (g.get_property(p1, "id"), g.get_property(p2, "id"), len(ms))
        for (p1, p2), ms in mutual.items()
    ]
    rows.sort(key=lambda r: (-r[2], r[0], r[1]))
    return rows[:20]

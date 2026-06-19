"""BI Q5 — most active posters of a topic.

For a tag, score each creator of the tag's messages by
1*messages + 2*replies + 10*likes-received; top 100 by score (person id
ascending on ties). Reports ``(person_id, messages, replies, likes, score)``.
Ids are the LDBC external ids (the ``id`` property).

Existing primitives: walk the tag's messages (incoming ``hasTag``), count each
message's incoming ``likes`` / ``replyOf`` with ``degree``, and attribute the
message + its reply/like tallies to its creator.
"""

from rustychickpeas import Direction


def q5_active_posters(g, tag_name: str):
    target = g.node_with_label_property("Tag", "name", tag_name)
    if target is None:
        return []

    agg = {}  # person -> [messages, replies, likes-received]
    for message in g.neighbor_ids(target, Direction.Incoming, ["hasTag"]):
        likes = g.degree(message, Direction.Incoming, "likes")
        replies = g.degree(message, Direction.Incoming, "replyOf")
        for person in g.neighbor_ids(message, Direction.Incoming, ["hasCreator"]):
            e = agg.get(person)
            if e is None:
                e = [0, 0, 0]
                agg[person] = e
            e[0] += 1
            e[1] += replies
            e[2] += likes

    rows = [
        (g.get_property(p, "id"), m, r, lk, m + 2 * r + 10 * lk)
        for p, (m, r, lk) in agg.items()
    ]
    rows.sort(key=lambda x: (-x[4], x[0]))
    return rows[:100]

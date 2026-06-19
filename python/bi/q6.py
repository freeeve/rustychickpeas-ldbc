"""BI Q6 — most authoritative users on a topic.

For each creator (person1) of a tag's messages, collect the people (person2)
who liked those messages, and score person1 by the total likes those person2s
received on their own messages. Top 100 by score (person id ascending on ties).
Reports ``(person_id, score)``; ids are the LDBC external ids (the ``id`` property).

Existing primitives, plus a memo: each person2's "likes received across their own
messages" is computed once (a liker recurs across many creators).
"""

from rustychickpeas import Direction


def q6_authoritative(g, tag_name: str):
    target = g.node_with_label_property("Tag", "name", tag_name)
    if target is None:
        return []

    p1_to_p2 = {}  # person1 -> set of distinct likers of person1's tagged messages
    for message1 in g.neighbor_ids(target, Direction.Incoming, ["hasTag"]):
        likers = g.neighbor_ids(message1, Direction.Incoming, ["likes"])
        if not likers:
            continue
        for person1 in g.neighbor_ids(message1, Direction.Incoming, ["hasCreator"]):
            p1_to_p2.setdefault(person1, set()).update(likers)

    likes_received = {}  # person2 -> total likes on the messages they created

    def received(p2):
        v = likes_received.get(p2)
        if v is None:
            v = sum(
                g.degree(m2, Direction.Incoming, "likes")
                for m2 in g.neighbor_ids(p2, Direction.Outgoing, ["hasCreator"])
            )
            likes_received[p2] = v
        return v

    rows = [
        (g.get_property(p1, "id"), sum(received(p2) for p2 in p2set))
        for p1, p2set in p1_to_p2.items()
    ]
    rows.sort(key=lambda x: (-x[1], x[0]))
    return rows[:100]

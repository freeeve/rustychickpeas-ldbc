"""BI Q3 — popular topics in a country.

For forums whose moderator lives in ``country``, count the distinct messages in
the forums' post reply-trees that carry a tag of ``tagclass``; top 20 by count
(forum id ascending on ties). Reports
``(forum_id, title, moderator_id, message_count)`` — fday is omitted (it's
deterministic from the forum id, so the cross-check uses id + moderator + count).

First cut: existing Python primitives — a small country→city→person→forum
traversal, then ``neighborhood`` to walk each post's reply-tree and a set of the
class's tags to test each message.
"""

from rustychickpeas import Direction

# replyOf is a finite tree, so a large bound just means "to the leaves".
_REPLY_DEPTH = 100_000


def q3_popular_topics(g, country_name: str, tagclass_name: str):
    country = g.node_with_label_property("Country", "name", country_name)
    tc = g.node_with_label_property("TagClass", "name", tagclass_name)
    if country is None or tc is None:
        return []

    class_tags = set(g.neighbor_ids(tc, Direction.Incoming, ["hasType"]))

    def has_class_tag(msg):
        return any(t in class_tags for t in g.neighbor_ids(msg, Direction.Outgoing, ["hasTag"]))

    rows = []
    for city in g.neighbor_ids(country, Direction.Incoming, ["isPartOf"]):
        for person in g.neighbor_ids(city, Direction.Incoming, ["isLocatedIn"]):
            for forum in g.neighbor_ids(person, Direction.Incoming, ["hasModerator"]):
                msgs = set()
                for post in g.neighbor_ids(forum, Direction.Outgoing, ["containerOf"]):
                    tree = g.neighborhood(post, Direction.Incoming, "replyOf", _REPLY_DEPTH)
                    for n in (post, *tree):
                        if has_class_tag(n):
                            msgs.add(n)
                if msgs:
                    rows.append(
                        (
                            g.node(forum).get_property("id"),
                            g.node(forum).get_property("title"),
                            g.node(person).get_property("id"),
                            len(msgs),
                        )
                    )
    rows.sort(key=lambda r: (-r[3], r[0]))
    return rows[:20]

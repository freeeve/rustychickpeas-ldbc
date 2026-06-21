"""BI Q3 — popular topics in a country.

For forums whose moderator lives in ``country``, count the distinct messages in
the forums' post reply-trees that carry a tag of ``tagclass``; top 20 by count
(forum id ascending on ties). Reports
``(forum_id, title, moderator_id, message_count)`` — fday is omitted (it's
deterministic from the forum id, so the cross-check uses id + moderator + count).

Investigated (task 169): the per-message ``hasTag`` check runs only over Burma's
small forum reply-trees, so it is already cheap (~14 ms). The obvious "flip" —
precomputing the graph-wide set of messages carrying any class tag — is a net loss
here (MusicalArtist spans millions of messages, so materializing that set dwarfs the
per-message checks; measured 40 ms). Beating ~14 ms would need a native "filter a
node set to those carrying any tag in S" kernel; left as the existing-primitive
version. Resolved a `Node` per forum once.
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
                    for n in (post, *g.neighborhood(post, Direction.Incoming, "replyOf", _REPLY_DEPTH)):
                        if has_class_tag(n):
                            msgs.add(n)
                if msgs:
                    fnode = g.node(forum)
                    rows.append(
                        (
                            fnode.get_property("id"),
                            fnode.get_property("title"),
                            g.node(person).get_property("id"),
                            len(msgs),
                        )
                    )
    rows.sort(key=lambda r: (-r[3], r[0]))
    return rows[:20]

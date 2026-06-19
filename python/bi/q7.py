"""BI Q7 — related topics.

For a tag, look at the comments replying to messages carrying that tag and, for
comments that don't themselves carry the tag, count distinct such comments per
*other* tag they carry. Top 100 by count (tag name ascending on ties). Reports
``(tag_name, comment_count)``.

Existing primitives: walk the tag's messages, their reply comments, and each
comment's other tags; dedupe comments per related tag with a set.
"""

from rustychickpeas import Direction


def q7_related_topics(g, tag_name: str):
    target = g.node_with_label_property("Tag", "name", tag_name)
    if target is None:
        return []

    related = {}  # other tag -> set of distinct reply comments carrying it
    for msg in g.neighbor_ids(target, Direction.Incoming, ["hasTag"]):
        for comment in g.neighbor_ids(msg, Direction.Incoming, ["replyOf"]):
            ctags = g.neighbor_ids(comment, Direction.Outgoing, ["hasTag"])
            if target not in ctags:
                for rt in ctags:
                    related.setdefault(rt, set()).add(comment)

    rows = [(g.prop_str(rt, "name") or "", len(cs)) for rt, cs in related.items()]
    rows.sort(key=lambda x: (-x[1], x[0]))
    return rows[:100]

"""IS7 — direct replies to a message: (reply, replyMs, replyAuthor, knows), where
``knows`` is True when the reply's author is a ``knows`` friend of the original
message's author (and not the same person). Ordered (replyMs desc, replyAuthor id asc).
"""

_NO_AUTHOR = 0xFFFFFFFF

from rustychickpeas import Direction


def is7_replies(g, message):
    author = g.first_neighbor(message, Direction.Incoming, "hasCreator")
    friends = (set(g.neighbor_ids(author, Direction.Outgoing, ["knows"]))
               if author is not None else set())
    rows = []
    for reply in g.neighbor_ids(message, Direction.Incoming, ["replyOf"]):
        ra = g.first_neighbor(reply, Direction.Incoming, "hasCreator")
        ra = _NO_AUTHOR if ra is None else ra
        knows = author is not None and author != ra and ra in friends
        rows.append((reply, g.get_property(reply, "ms"), ra, knows))
    rows.sort(key=lambda r: (-r[1], g.get_property(r[2], "id") if r[2] != _NO_AUTHOR else 0))
    return rows

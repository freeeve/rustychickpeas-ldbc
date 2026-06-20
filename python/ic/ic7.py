"""IC7 — the 20 most recent likers of the start person's messages, keeping the latest
like per liker, ordered (likeTime desc, liker id asc). Returns
(liker, like_ms, message, is_new), where is_new = the liker is not a ``knows`` friend.
"""

from rustychickpeas import Direction


def ic7_recent_likers(g, person):
    friends = set(g.neighbor_ids(person, Direction.Outgoing, ["knows"]))
    best = {}  # liker -> (like_ms, message); likes is Person->Message, liker = source
    for msg in g.neighbor_ids(person, Direction.Outgoing, ["hasCreator"]):
        for rel in g.relationships(msg, Direction.Incoming, ["likes"]):
            liker = rel.start_node().id()
            lms = rel.get_property("ld") or 0
            cur = best.get(liker)
            if cur is None or lms > cur[0] or (lms == cur[0] and msg < cur[1]):
                best[liker] = (lms, msg)
    rows = [(liker, lms, msg, liker not in friends) for liker, (lms, msg) in best.items()]
    rows.sort(key=lambda r: (-r[1], g.get_property(r[0], "id")))
    return rows[:20]

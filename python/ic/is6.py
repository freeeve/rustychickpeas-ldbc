"""IS6 — the (forum, moderator) containing a message's thread. ``roots`` is the
``roots_via("replyOf", Outgoing)`` NodeArray (each message -> its root Post, a Post
maps to itself); one ``containerOf`` hop reaches the forum, then ``hasModerator``.
"""

from rustychickpeas import Direction


def is6_forum_of_message(g, message, roots):
    root = roots[message]
    forum = g.first_neighbor(root, Direction.Incoming, "containerOf")
    if forum is None:
        return None
    moderator = g.first_neighbor(forum, Direction.Outgoing, "hasModerator")
    if moderator is None:
        return None
    return (forum, moderator)

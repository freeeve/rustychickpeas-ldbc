"""BI Q9 — top thread initiators.

For each person, count their Posts created in a day window (threads) and the
messages in those posts' reply trees that also fall in the window. Top 100 by
message count (person id ascending on ties). Reports
``(person_id, threads, messages)``.

Both the window's Posts and Comments come straight from the day index (one exact
lookup per day). Instead of walking each post's reply tree *down* (which scans
like-heavy posts' incoming replyOf), we walk each window comment *up* its replyOf
chain (one cheap outgoing edge per hop, memoized) to its thread root and, if that
root is a window post, credit the root's creator. A reply is always later than its
parent, so a window comment's whole ancestor chain is also in-window — no day
checks needed once we've filtered by the index.
"""

from rustychickpeas import Direction


def q9_thread_initiators(g, start_day: int, end_day: int):
    days = range(start_day, end_day + 1)

    threads = {}            # creator -> thread (window-post) count
    messages = {}           # creator -> in-window message count
    post_creator = {}       # window post -> its creator
    roots = {}              # message -> thread root (memoized; window posts seed it)
    for d in days:
        for post in g.nodes_with_property("Post", "day", d):
            creator = g.first_neighbor(post, Direction.Incoming, "hasCreator")
            if creator is None:
                continue
            post_creator[post] = creator
            roots[post] = post
            threads[creator] = threads.get(creator, 0) + 1
            messages[creator] = messages.get(creator, 0) + 1  # the post itself

    def root_of(m):
        path = []
        cur = m
        while cur not in roots:
            parent = g.first_neighbor(cur, Direction.Outgoing, "replyOf")
            if parent is None:
                roots[cur] = cur
                break
            path.append(cur)
            cur = parent
        r = roots[cur]
        for n in path:
            roots[n] = r
        return r

    for d in days:
        for comment in g.nodes_with_property("Comment", "day", d):
            creator = post_creator.get(root_of(comment))
            if creator is not None:
                messages[creator] += 1

    rows = [(g.get_property(p, "id"), threads[p], messages[p]) for p in threads]
    rows.sort(key=lambda r: (-r[2], r[0]))
    return rows[:100]

"""BI Q9 — top thread initiators.

For each person, count their Posts created in a day window (threads) and the
messages in those posts' reply trees that also fall in the window. Top 100 by
message count (person id ascending on ties). Reports
``(person_id, threads, messages)``.

Both the window's Posts and Comments come straight from the day index. Each window
comment is credited to its thread root's creator if that root is a window post — and
the thread root is read from the native ``roots_via("replyOf")`` forest-root array
(O(1) per comment, built once) instead of walking the replyOf chain in Python. A
reply is later than its parent, so a window comment's ancestor chain is also
in-window: no day checks needed once filtered by the index.
"""

from rustychickpeas import Direction


def q9_thread_initiators(g, start_day: int, end_day: int):
    days = range(start_day, end_day + 1)
    roots = memoryview(g.roots_via("replyOf", Direction.Outgoing))  # message -> thread root

    threads = {}       # creator -> thread (window-post) count
    messages = {}      # creator -> in-window message count
    post_creator = {}  # window post -> its creator
    for d in days:
        for post in g.nodes_with_property("Post", "day", d):
            creator = g.first_neighbor(post, Direction.Incoming, "hasCreator")
            if creator is None:
                continue
            post_creator[post] = creator
            threads[creator] = threads.get(creator, 0) + 1
            messages[creator] = messages.get(creator, 0) + 1  # the post itself

    for d in days:
        for comment in g.nodes_with_property("Comment", "day", d):
            creator = post_creator.get(roots[comment])
            if creator is not None:
                messages[creator] += 1

    rows = [(g.get_property(p, "id"), threads[p], messages[p]) for p in threads]
    rows.sort(key=lambda r: (-r[2], r[0]))
    return rows[:100]

"""BI Q9 — top thread initiators.

For each person, count their Posts created in a day window (threads) and the
messages in those posts' reply trees that also fall in the window. Top 100 by
message count (person id ascending on ties). Reports
``(person_id, threads, messages)``.

Existing primitives: scan Posts in the window, attribute each to its creator,
and DFS the reply tree pruning any node past ``end_day`` (replies are later than
their parent, so the whole subtree is later). The day filter reads the dense
``day`` column through the buffer protocol (O(1) per node, no per-node call) so
the ~1.1M-Post scan doesn't pay a property lookup each step.
"""

from rustychickpeas import Direction

import columns


def q9_thread_initiators(g, start_day: int, end_day: int):
    day_at = columns.i64_reader(g, "day")
    per_person = {}  # person -> [threads, messages]
    # Fetch the window's Posts straight from the day index (one exact-value lookup
    # per day) rather than scanning all ~1.1M Posts.
    window_posts = (p for d in range(start_day, end_day + 1)
                    for p in g.nodes_with_property("Post", "day", d))
    for post in window_posts:
        creator = g.first_neighbor(post, Direction.Incoming, "hasCreator")
        if creator is None:
            continue

        msgs = 0
        stack = [post]
        while stack:
            n = stack.pop()
            d = day_at(n)
            if d > end_day:
                continue
            if d >= start_day:
                msgs += 1
            stack.extend(g.neighbor_ids(n, Direction.Incoming, ["replyOf"]))

        e = per_person.get(creator)
        if e is None:
            e = [0, 0]
            per_person[creator] = e
        e[0] += 1
        e[1] += msgs

    rows = [(g.get_property(p, "id"), t, m) for p, (t, m) in per_person.items()]
    rows.sort(key=lambda x: (-x[2], x[0]))
    return rows[:100]

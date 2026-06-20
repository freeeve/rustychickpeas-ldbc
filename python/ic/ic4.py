"""IC4 — "new topics": Tags on the seed's friends' Posts created within
``[start_day, start_day + duration_days)`` that were never on those friends' Posts
before ``start_day``. Returns (tag_id, post_count), (count desc, name asc), top 10.
"""

from rustychickpeas import Direction

from ._cols import i64_reader


def ic4_new_topics(g, person, start_day, duration_days):
    end_day = start_day + duration_days
    posts = set(g.nodes_with_label("Post"))
    day = i64_reader(g, "day")
    in_window, before = {}, set()
    for friend in g.neighbor_ids(person, Direction.Outgoing, ["knows"]):
        for post in g.neighbor_ids(friend, Direction.Outgoing, ["hasCreator"]):
            if post not in posts:
                continue
            d = day(post)
            if d < start_day:
                for t in g.neighbor_ids(post, Direction.Outgoing, ["hasTag"]):
                    before.add(t)
            elif d < end_day:
                for t in g.neighbor_ids(post, Direction.Outgoing, ["hasTag"]):
                    in_window[t] = in_window.get(t, 0) + 1
    rows = [(t, c) for t, c in in_window.items() if t not in before]
    rows.sort(key=lambda r: (-r[1], g.prop_str(r[0], "name") or ""))
    return rows[:10]

"""IS2 — a person's own 10 most recent messages on/before max_day.

Returns ``(message_id, ms)`` by (ms desc, id asc), top 10.
"""

from rustychickpeas import Direction


def is2_recent_of_person(g, person, max_day):
    cands = []
    for m in g.neighbor_ids(person, Direction.Outgoing, ["hasCreator"]):
        if g.get_property(m, "day") <= max_day:
            cands.append((m, g.get_property(m, "ms")))
    cands.sort(key=lambda x: (-x[1], x[0]))
    return cands[:10]

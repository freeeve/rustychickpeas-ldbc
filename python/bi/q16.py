"""BI Q16 — fake news detection.

For a (tag, day) param, find people who made a message with that tag on that day
and have at most ``max_knows`` friends who did the same, with their message count.
Q16 returns people qualifying for BOTH params, ranked by combined message count
(person id ascending on ties), top 20. Reports ``(person_id, count_a, count_b)``.

Existing primitives: per-param, tally tagged-on-day message creators and keep those
with few same-day-tagging friends; then intersect the two params.
"""

from rustychickpeas import Direction


def q16_param_result(g, tag_name: str, day: int, max_knows: int):
    """Map person -> their tagged-on-day message count, kept only if at most
    ``max_knows`` of their friends also posted with the tag that day."""
    tag = g.node_with_label_property("Tag", "name", tag_name)
    if tag is None:
        return {}

    cm = {}
    creators_on_day = set()
    for msg in g.neighbor_ids(tag, Direction.Incoming, ["hasTag"]):
        if (g.get_property(msg, "day") or 0) != day:
            continue
        for creator in g.neighbor_ids(msg, Direction.Incoming, ["hasCreator"]):
            cm[creator] = cm.get(creator, 0) + 1
            creators_on_day.add(creator)

    return {
        p: c
        for p, c in cm.items()
        if sum(1 for f in g.neighbor_ids(p, Direction.Outgoing, ["knows"]) if f in creators_on_day)
        <= max_knows
    }


def q16_fake_news(g, ra, rb):
    """Combine two ``q16_param_result`` maps: people in both, top 20 by ca+cb."""
    rows = [(g.get_property(p, "id"), ca, rb[p]) for p, ca in ra.items() if p in rb]
    rows.sort(key=lambda r: (-(r[1] + r[2]), r[0]))
    return rows[:20]

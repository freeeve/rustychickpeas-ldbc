"""BI Q8 — central person for a tag.

Score each person by 100*interest-in-the-tag + messages they made carrying the
tag in a day window, then add their friends' base scores. Top 100 by
score+friendsScore (person id ascending on ties). Reports
``(person_id, score, friends_score)``.

Existing primitives: a base-score map (hasInterest gives 100, each in-window
tagged message gives 1), then a knows-neighbor sum for the friends' score.
"""

from rustychickpeas import Direction


def q8_central_person(g, tag_name: str, start_day: int, end_day: int):
    tag = g.node_with_label_property("Tag", "name", tag_name)
    if tag is None:
        return []

    score = {}  # person -> base score (only interested / in-window creators appear)
    for p in g.neighbor_ids(tag, Direction.Incoming, ["hasInterest"]):
        score[p] = score.get(p, 0) + 100
    for msg in g.neighbor_ids(tag, Direction.Incoming, ["hasTag"]):
        day = g.get_property(msg, "day") or 0
        if start_day < day < end_day:
            for creator in g.neighbor_ids(msg, Direction.Incoming, ["hasCreator"]):
                score[creator] = score.get(creator, 0) + 1

    rows = []
    for p, s in score.items():
        fs = sum(score.get(f, 0) for f in g.neighbor_ids(p, Direction.Outgoing, ["knows"]))
        rows.append((g.get_property(p, "id"), s, fs))
    rows.sort(key=lambda x: (-(x[1] + x[2]), x[0]))
    return rows[:100]

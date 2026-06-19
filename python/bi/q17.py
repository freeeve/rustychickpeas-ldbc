"""BI Q17 — information propagation.

For a tag, count distinct message2 per person1 where: person1's tagged message1
sits in forum1; a forum1 member (person2) posted a tagged comment replying to
message2 (by a different forum1 member person3, also tagged) in a different forum2;
message2 is more than ``delta_hours`` after message1; and person1 is not a forum2
member. Top 10 by count. Reports ``(person_id, message_count)``.

Existing primitives plus memoized thread roots (forum of a message = the forum of
its thread root post) and per-person forum-membership sets.
"""

from rustychickpeas import Direction


def q17_information_propagation(g, tag_name: str, delta_hours: int):
    tag = g.node_with_label_property("Tag", "name", tag_name)
    if tag is None:
        return []
    delta_ms = delta_hours * 3_600_000

    roots = {}
    root_forum = {}

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

    def forum_of(m):
        root = root_of(m)
        if root not in root_forum:
            root_forum[root] = g.first_neighbor(root, Direction.Incoming, "containerOf")
        return root_forum[root]

    tagged = list(g.neighbor_ids(tag, Direction.Incoming, ["hasTag"]))
    tagged_set = set(tagged)

    m1_list = []  # (p1, f1, ms1)
    cand = []     # (p2, p3, msg2, f2, ms2)
    for m in tagged:
        p1, f1 = g.first_neighbor(m, Direction.Incoming, "hasCreator"), forum_of(m)
        if p1 is not None and f1 is not None:
            m1_list.append((p1, f1, g.get_property(m, "ms") or 0))
        msg2 = g.first_neighbor(m, Direction.Outgoing, "replyOf")
        if msg2 is not None and msg2 in tagged_set:
            p2 = g.first_neighbor(m, Direction.Incoming, "hasCreator")
            p3 = g.first_neighbor(msg2, Direction.Incoming, "hasCreator")
            f2 = forum_of(msg2)
            if p2 is not None and p3 is not None and f2 is not None:
                cand.append((p2, p3, msg2, f2, g.get_property(msg2, "ms") or 0))

    # Forum membership, built from the FORUM side. A person's incoming hasMember is
    # buried among their (knows-heavy) incoming edges, but each forum in play has few
    # outgoing hasMember edges -> build forum->members for the forums that appear in
    # m1/cand and invert. Only relevant-forum memberships are needed: the join only
    # ever tests f1 (from m1) and f2 (from cand).
    relevant = {f1 for _, f1, _ in m1_list} | {f2 for _, _, _, f2, _ in cand}
    pm = {}  # person -> set of relevant forums they belong to
    for f in relevant:
        for p in g.neighbor_ids(f, Direction.Outgoing, ["hasMember"]):
            pm.setdefault(p, set()).add(f)

    # Index m1 by its forum so each candidate scans only the m1 entries whose forum
    # both p2 and p3 belong to (pm[p2] & pm[p3]), not the whole m1 list.
    m1_by_forum = {}
    for p1, f1, ms1 in m1_list:
        m1_by_forum.setdefault(f1, []).append((p1, ms1))

    _EMPTY = frozenset()
    counts = {}  # person1 -> set of distinct message2
    for p2, p3, msg2, f2, ms2 in cand:
        if p2 == p3:
            continue
        for f1 in pm.get(p2, _EMPTY) & pm.get(p3, _EMPTY):
            if f1 == f2:
                continue
            for p1, ms1 in m1_by_forum.get(f1, ()):
                if ms2 > ms1 + delta_ms and f2 not in pm.get(p1, _EMPTY):
                    counts.setdefault(p1, set()).add(msg2)

    rows = [(g.get_property(p, "id"), len(m)) for p, m in counts.items()]
    rows.sort(key=lambda r: (-r[1], r[0]))
    return rows[:10]

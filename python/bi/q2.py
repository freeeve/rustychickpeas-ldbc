"""BI Q2 — tag evolution.

For the tags of a given TagClass, count messages tagged with them in two
consecutive 100-day windows from ``date0_day``, and report
``(tag_name, count_w1, count_w2, abs_diff)`` (top 100 by diff desc, name asc).

First cut: existing Python primitives (a per-message loop traversing ``hasTag``).
This is the slow path that motivates a core neighbor-grouped aggregation.
"""

from rustychickpeas import Direction


def q2_tag_evolution(g, date0_day: int, class_name: str):
    target = g.node_with_label_property("TagClass", "name", class_name)
    if target is None:
        return []

    # Tags whose hasType points at the target TagClass.
    qualifying = set()
    for t in g.nodes_with_label("Tag"):
        if target in g.neighbor_ids(t, Direction.Outgoing, ["hasType"]):
            qualifying.add(t)

    w1_lo, w1_hi = date0_day, date0_day + 100
    w2_lo, w2_hi = date0_day + 100, date0_day + 200
    day = g.column("day").to_pylist()

    c1, c2 = {}, {}
    for label in ("Post", "Comment"):
        for msg in g.nodes_with_label(label):
            d = day[msg]
            in1 = w1_lo <= d < w1_hi
            in2 = w2_lo <= d < w2_hi
            if not in1 and not in2:
                continue
            counts = c1 if in1 else c2
            for t in g.neighbor_ids(msg, Direction.Outgoing, ["hasTag"]):
                if t in qualifying:
                    counts[t] = counts.get(t, 0) + 1

    rows = []
    for t in qualifying:
        n1 = c1.get(t, 0)
        n2 = c2.get(t, 0)
        name = g.node(t).get_property("name")
        rows.append((name, n1, n2, abs(n1 - n2)))
    rows.sort(key=lambda r: (-r[3], r[0]))
    return rows[:100]


def _qualifying_tags(g, class_name):
    target = g.node_with_label_property("TagClass", "name", class_name)
    if target is None:
        return None
    # Tags of the class are exactly the target's incoming hasType neighbors —
    # one traversal instead of scanning every Tag.
    return set(g.neighbor_ids(target, Direction.Incoming, ["hasType"]))


def q2_tag_evolution_native(g, date0_day: int, class_name: str):
    """Q2 via the core aggregation builder: the per-message hasTag traversal,
    window binning and per-(window, tag) counting all run in Rust (GIL released)
    through ``.through("hasTag", ...)``; Python only filters to qualifying tags and
    reshapes the ~32k (window, tag) rows."""
    qualifying = _qualifying_tags(g, class_name)
    if qualifying is None:
        return []

    w1_hi = date0_day + 100
    res = (
        g.aggregate("Post", "Comment")
        .where("day", ">=", date0_day)
        .where("day", "<", date0_day + 200)
        .bin("day", [w1_hi])  # day_bin 0 = window 1, 1 = window 2
        .through("hasTag", Direction.Outgoing)
        .only_neighbors(list(qualifying))  # count only this class's tags, in core
        .run()
    )

    c1, c2 = {}, {}
    for r in res.rows:
        t = r["neighbor"]
        (c1 if r["day_bin"] == 0 else c2)[t] = r["count"]

    rows = []
    for t in qualifying:
        n1 = c1.get(t, 0)
        n2 = c2.get(t, 0)
        name = g.node(t).get_property("name")
        rows.append((name, n1, n2, abs(n1 - n2)))
    rows.sort(key=lambda r: (-r[3], r[0]))
    return rows[:100]

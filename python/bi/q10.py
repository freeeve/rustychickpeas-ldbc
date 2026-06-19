"""BI Q10 — experts in a country.

From a start person, find experts at knows hop-distance in [min_dist, max_dist]
who live in ``country`` and created messages tagged with a tag of ``tagclass``;
count distinct messages per (expert, tag). Top 100 by count (tag name then
person id ascending on ties). Reports ``(person_id, tag_name, message_count)``.

Existing primitives: the native ``bfs_distances`` for the bounded knows BFS, then
country membership + class-tag sets, and a distinct-message count per (expert, tag).
"""

from rustychickpeas import Direction


def q10_experts(g, start_plid, country_name, tagclass_name, min_dist, max_dist):
    start = g.node_with_label_property("Person", "id", start_plid)
    if start is None:
        return []
    dist = g.bfs_distances(start, Direction.Outgoing, rel_types=["knows"], max_depth=max_dist)

    country = g.node_with_label_property("Country", "name", country_name)
    tc = g.node_with_label_property("TagClass", "name", tagclass_name)
    if country is None or tc is None:
        return []

    in_country = set()
    for city in g.neighbor_ids(country, Direction.Incoming, ["isPartOf"]):
        in_country.update(g.neighbor_ids(city, Direction.Incoming, ["isLocatedIn"]))
    class_tags = set(g.neighbor_ids(tc, Direction.Incoming, ["hasType"]))

    counts = {}  # (expert, tag) -> set of distinct messages
    for expert, d in dist.items():
        if d < min_dist or d > max_dist or expert not in in_country:
            continue
        for msg in g.neighbor_ids(expert, Direction.Outgoing, ["hasCreator"]):
            tags = g.neighbor_ids(msg, Direction.Outgoing, ["hasTag"])
            if any(t in class_tags for t in tags):
                for t in tags:
                    counts.setdefault((expert, t), set()).add(msg)

    rows = [
        (g.get_property(e, "id"), g.prop_str(t, "name") or "", len(msgs))
        for (e, t), msgs in counts.items()
    ]
    rows.sort(key=lambda x: (-x[2], x[1], x[0]))
    return rows[:100]

"""BI Q12 — message-count histogram.

Per person, count their messages with content, length < len_thr, created after
min_day, whose thread root Post's language is in ``langs``; then histogram persons
by that count (including the zero bucket). Sorted by person count descending, then
message count descending. Reports ``(message_count, person_count)``.

Fast for Python: the day/len/content filters read dense i64 columns through the
buffer protocol (O(1), no per-node call); the thread root is found by a memoized
walk up the replyOf chain.
"""

from rustychickpeas import Direction


def _i64_reader(g, key):
    """O(1) reader for a dense i64 column via the buffer protocol, falling back to
    per-node ``get_property`` when the column isn't dense/bufferable."""
    col = g.column(key)
    if col is not None:
        try:
            mv = memoryview(col)
            return lambda m: mv[m]
        except (TypeError, ValueError):
            pass
    return lambda m: g.get_property(m, key) or 0


def q12_message_counts(g, min_day, len_thr, langs):
    langs = set(langs)
    day_at = _i64_reader(g, "day")
    len_at = _i64_reader(g, "len")
    content_at = _i64_reader(g, "content")

    roots = {}  # message -> thread root (terminal of its replyOf chain)

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

    per_person = {}
    for label in ("Post", "Comment"):
        for msg in g.nodes_with_label(label):
            if day_at(msg) <= min_day or not content_at(msg) or len_at(msg) >= len_thr:
                continue
            if g.prop_str(root_of(msg), "lang") not in langs:
                continue
            for creator in g.neighbor_ids(msg, Direction.Incoming, ["hasCreator"]):
                per_person[creator] = per_person.get(creator, 0) + 1

    total_persons = len(g.nodes_with_label("Person"))
    hist = {}
    for c in per_person.values():
        hist[c] = hist.get(c, 0) + 1
    hist[0] = total_persons - len(per_person)
    return sorted(hist.items(), key=lambda x: (-x[1], -x[0]))

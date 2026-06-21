"""BI Q12 — message-count histogram.

Per person, count their messages with content, length < len_thr, created after
min_day, whose thread root Post's language is in ``langs``; then histogram persons
by that count (including the zero bucket). Sorted by person count descending, then
message count descending. Reports ``(message_count, person_count)``.

Optimization (task 178): the whole 2.8M-message scan runs in the native parallel
`aggregate` kernel. The day/content/len scalar filters, the new `where_via` (the
thread-root's `lang` ∈ ``langs``, via the `roots_via` replyOf projection), and the
per-creator count (`through("hasCreator")`) all run in Rust with the GIL released;
only the small histogram + zero-bucket + sort stay in Python.
"""

from rustychickpeas import Direction


def q12_message_counts(g, min_day, len_thr, langs):
    roots = g.roots_via("replyOf", Direction.Outgoing)  # message -> thread root
    rows = (
        g.aggregate("Post", "Comment")
        .where("day", ">", min_day)
        .where("content", "==", 1)
        .where("len", "<", len_thr)
        .where_via(roots, "lang", list(langs))
        .through("hasCreator", Direction.Incoming)  # count per creator
        .run()
        .rows
    )  # [{neighbor: creator, count: n}], one per creator with >=1 qualifying message

    total_persons = len(g.nodes_with_label("Person"))
    hist = {}
    for r in rows:
        c = r["count"]
        hist[c] = hist.get(c, 0) + 1
    hist[0] = total_persons - len(rows)
    return sorted(hist.items(), key=lambda x: (-x[1], -x[0]))

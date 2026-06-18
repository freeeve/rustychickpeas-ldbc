"""BI Q1 — posting summary.

Group messages created before ``cutoff_day`` (that have content) by
``(year, isComment, lengthCategory)`` and report count + total length.

Two implementations:

* ``q1_posting_summary`` — readable reference: export columns via
  ``column(key).to_pylist()`` and run a tight Python loop with dict aggregation.
* ``q1_posting_summary_arrow`` — optimized: wrap the dense columns as zero-copy
  pyarrow arrays (``columns.arrow`` over the buffer-protocol ``column(key)``) and
  do the filter / category / group-by entirely in vectorized pyarrow compute.

``load_messages`` lays Post nodes out before Comment nodes in one contiguous id
range each, so the arrow path slices ``[0, post_count)`` / ``[post_count, n)``
rather than materializing per-label id lists.
"""

import pyarrow as pa
import pyarrow.compute as pc

import columns


def _length_category(ln: int) -> int:
    if ln < 40:
        return 0
    if ln < 80:
        return 1
    if ln < 160:
        return 2
    return 3


def q1_posting_summary(g, cutoff_day: int):
    """Return ``(rows, total)`` where each row is
    ``(year, is_comment, length_category, message_count, sum_length)`` sorted by
    year desc, is_comment asc, category asc, and ``total`` is the number of
    messages before the cutoff (content or not)."""
    day = g.column("day").to_pylist()
    length = g.column("len").to_pylist()
    year = g.column("year").to_pylist()
    content = g.column("content").to_pylist()

    groups = {}
    total = 0
    for label, is_comment in (("Post", False), ("Comment", True)):
        for n in g.nodes_with_label(label):
            if day[n] >= cutoff_day:
                continue
            total += 1
            if not content[n]:
                continue
            ln = length[n]
            key = (year[n], is_comment, _length_category(ln))
            cnt, sum_len = groups.get(key, (0, 0))
            groups[key] = (cnt + 1, sum_len + ln)

    rows = [
        (y, is_comment, cat, cnt, sum_len)
        for (y, is_comment, cat), (cnt, sum_len) in groups.items()
    ]
    rows.sort(key=lambda r: (-r[0], r[1], r[2]))
    return rows, total


def q1_posting_summary_native(g, cutoff_day: int):
    """Vectorized Q1 done entirely in rustychickpeas's Rust (GIL released), no
    numpy/pyarrow, via the fluent aggregation builder. Python only reshapes the
    ~12 self-describing result rows."""
    res = (
        g.aggregate("Post", "Comment")
        .where("day", "<", cutoff_day)  # population -> res.total
        .having("content", "!=", 0)  # extra filter for grouped rows
        .by_label()  # group by source label (name)
        .by("year")
        .bin("len", [40, 80, 160])  # length category 0..3 -> "len_bin"
        .sum("len")
        .run()
    )
    out = [
        (r["year"], r["label"] == "Comment", r["len_bin"], r["count"], r["sum"])
        for r in res.rows
    ]
    out.sort(key=lambda r: (-r[0], r[1], r[2]))
    return out, res.total


def _category_arrow(ln):
    """Vectorized length category: 0/1/2/3 by thresholds 40/80/160."""
    return pc.add(
        pc.add(
            pc.cast(pc.greater_equal(ln, 40), pa.int64()),
            pc.cast(pc.greater_equal(ln, 80), pa.int64()),
        ),
        pc.cast(pc.greater_equal(ln, 160), pa.int64()),
    )


def q1_posting_summary_arrow(g, cutoff_day: int, post_count: int):
    """Vectorized Q1. ``post_count`` is the number of Post nodes (Posts occupy
    ids ``[0, post_count)``, Comments ``[post_count, node_count)``)."""
    day = columns.arrow(g, "day")
    length = columns.arrow(g, "len")
    year = columns.arrow(g, "year")
    content = columns.arrow(g, "content")
    n = len(day)

    rows = []
    total = 0
    for lo, hi, is_comment in ((0, post_count, False), (post_count, n, True)):
        span = hi - lo
        before = pc.less(day.slice(lo, span), cutoff_day)
        total += pc.sum(pc.cast(before, pa.int64())).as_py() or 0
        keep = pc.and_(before, pc.not_equal(content.slice(lo, span), 0))
        ln = pc.filter(length.slice(lo, span), keep)
        table = pa.table(
            {
                "year": pc.filter(year.slice(lo, span), keep),
                "cat": _category_arrow(ln),
                "len": ln,
            }
        )
        agg = table.group_by(["year", "cat"]).aggregate([("len", "count"), ("len", "sum")])
        for y, cat, cnt, sum_len in zip(
            agg.column("year").to_pylist(),
            agg.column("cat").to_pylist(),
            agg.column("len_count").to_pylist(),
            agg.column("len_sum").to_pylist(),
        ):
            rows.append((y, is_comment, cat, cnt, sum_len))

    rows.sort(key=lambda r: (-r[0], r[1], r[2]))
    return rows, total

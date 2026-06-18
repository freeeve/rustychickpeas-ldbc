"""Tests for the Python LDBC BI loader + queries on tiny synthetic data."""

import gzip
import os

import props
import loader
from bi import q1


def test_days_from_civil_known_values():
    assert props.days_from_civil(1970, 1, 1) == 0
    assert props.days_from_civil(1970, 1, 2) == 1
    assert props.days_from_civil(2000, 1, 1) == 10957
    assert props.days_from_civil(2011, 12, 1) == 15309


def test_parse_date():
    assert props.parse_date("2010-02-24T08:06:02.996+00:00") == (
        2010,
        props.days_from_civil(2010, 2, 24),
    )
    assert props.parse_date("bad") is None


def _write_part(entity_dir, rows):
    """Write one ``part-00000.csv.gz`` with the LDBC message columns the loader
    reads (id|creationDate|content|length). ``rows`` are (id, date, content, length)."""
    os.makedirs(entity_dir, exist_ok=True)
    path = os.path.join(entity_dir, "part-00000.csv.gz")
    with gzip.open(path, "wt", newline="", encoding="utf-8") as fh:
        fh.write("id|creationDate|content|length\n")
        for ext_id, date, content, length in rows:
            fh.write(f"{ext_id}|{date}|{content}|{length}\n")


def test_q1_posting_summary(tmp_path):
    dynamic = tmp_path / "dynamic"
    _write_part(
        str(dynamic / "Post"),
        [
            (100, "2010-01-01T00:00:00.000+00:00", "hello", 10),  # 2010 cat0
            (101, "2011-06-01T00:00:00.000+00:00", "", 50),  # no content -> total only
            (102, "2012-01-01T00:00:00.000+00:00", "x", 100),  # after cutoff -> excluded
        ],
    )
    _write_part(
        str(dynamic / "Comment"),
        [
            (200, "2011-01-01T00:00:00.000+00:00", "abc", 45),  # 2011 Comment cat1
            (201, "2010-05-05T00:00:00.000+00:00", "yo", 200),  # 2010 Comment cat3
        ],
    )

    g, stats = loader.load_messages(str(tmp_path))
    assert stats == {"posts": 3, "comments": 2}
    assert g.node_count() == 5

    cutoff = props.days_from_civil(2011, 12, 1)
    rows, total = q1.q1_posting_summary(g, cutoff)

    # P1, P2, C1, C2 are before the cutoff (P3 is after); P2 has no content.
    assert total == 4
    assert rows == [
        (2011, True, 1, 1, 45),
        (2010, False, 0, 1, 10),
        (2010, True, 3, 1, 200),
    ]

    # The vectorized pyarrow path and the native Rust kernel must both agree.
    rows_arrow, total_arrow = q1.q1_posting_summary_arrow(g, cutoff, stats["posts"])
    assert (rows_arrow, total_arrow) == (rows, total)
    rows_native, total_native = q1.q1_posting_summary_native(g, cutoff)
    assert (rows_native, total_native) == (rows, total)

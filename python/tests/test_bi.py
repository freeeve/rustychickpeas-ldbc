"""Tests for the Python LDBC BI loader + queries on tiny synthetic data."""

import gzip
import os

import props
import loader
from bi import q1, q3, q4
from rustychickpeas import GraphSnapshotBuilder


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


def test_q3_popular_topics():
    # country <- city <- person (moderator) <- forum -> post <- comment(replyOf);
    # post & comment both carry a tag of the class.
    b = GraphSnapshotBuilder()
    nodes = [
        (0, "Country"), (1, "City"), (2, "Person"), (3, "Forum"),
        (4, "Post"), (5, "Comment"), (6, "TagClass"), (7, "Tag"),
    ]
    for nid, label in nodes:
        b.add_node([label], node_id=nid)
    b.set_prop(0, "name", "X")
    b.set_prop(6, "name", "TC")
    b.set_prop(3, "id", 10)
    b.set_prop(3, "title", "F")
    b.set_prop(2, "id", 1)
    for u, v, rel in [
        (1, 0, "isPartOf"), (2, 1, "isLocatedIn"), (3, 2, "hasModerator"),
        (3, 4, "containerOf"), (5, 4, "replyOf"), (7, 6, "hasType"),
        (4, 7, "hasTag"), (5, 7, "hasTag"),
    ]:
        b.add_relationship(u, v, rel)
    g = b.finalize()

    rows = q3.q3_popular_topics(g, "X", "TC")
    # forum 10, moderator 1, both post(4) and comment(5) class-tagged -> count 2.
    assert rows == [(10, "F", 1, 2)]


def test_q4_top_creators():
    # F1 (after cutoff) has members in two countries; its post reply-tree is
    # authored by those members. F2 (before cutoff) is excluded, so its lone
    # member must not appear.
    b = GraphSnapshotBuilder()
    nodes = [
        (0, "Country"), (1, "Country"), (2, "City"), (3, "City"),
        (4, "Person"), (5, "Person"), (6, "Person"), (7, "Person"),
        (8, "Forum"), (9, "Forum"),
        (10, "Post"), (11, "Comment"), (12, "Comment"),
    ]
    for nid, label in nodes:
        b.add_node([label], node_id=nid)
    for nid, ext in [(0, 100), (1, 200), (4, 1), (5, 2), (6, 3), (7, 4), (8, 10), (9, 11)]:
        b.set_prop(nid, "id", ext)
    b.set_prop(8, "fday", 200)  # F1 after cutoff (100)
    b.set_prop(9, "fday", 50)   # F2 before cutoff -> excluded
    edges = [
        (2, 0, "isPartOf"), (3, 1, "isPartOf"),
        (4, 2, "isLocatedIn"), (5, 2, "isLocatedIn"),
        (6, 3, "isLocatedIn"), (7, 3, "isLocatedIn"),
        (8, 4, "hasMember"), (8, 5, "hasMember"), (8, 6, "hasMember"),
        (9, 7, "hasMember"),  # F2's only member (forum excluded)
        (8, 10, "containerOf"),
        (11, 10, "replyOf"), (12, 11, "replyOf"),
        (6, 10, "hasCreator"), (4, 11, "hasCreator"), (5, 12, "hasCreator"),
    ]
    for u, v, rel in edges:
        b.add_relationship(u, v, rel)
    g = b.finalize()

    rows, top_ids = q4.q4_top_creators(g, after_day=100)
    assert top_ids == [10]
    # Post(P3), C1(P1), C2(P2) each authored one message -> all count 1, id asc.
    assert rows == [(1, 1), (2, 1), (3, 1)]

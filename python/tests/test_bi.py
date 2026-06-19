"""Tests for the Python LDBC BI loader + queries on tiny synthetic data."""

import gzip
import os

import props
import loader
from bi import q1, q3, q4, q5, q6, q7, q8, q9
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


def test_q5_active_posters():
    # Tag T on two messages, both created by P1; M1 has 2 likes + 1 reply, M2 has
    # 1 like. P1 aggregates msgs=2, replies=1, likes=3 -> score 2 + 2 + 30 = 34.
    b = GraphSnapshotBuilder()
    nodes = [
        (0, "Tag"), (1, "Person"), (2, "Person"), (3, "Person"), (4, "Person"),
        (5, "Post"), (6, "Comment"),
    ]
    for nid, label in nodes:
        b.add_node([label], node_id=nid)
    b.set_prop(0, "name", "T")
    for nid, ext in [(1, 1), (2, 2), (3, 3), (4, 4)]:
        b.set_prop(nid, "id", ext)
    edges = [
        (5, 0, "hasTag"), (6, 0, "hasTag"),
        (1, 5, "hasCreator"), (1, 6, "hasCreator"),  # P1 created both
        (6, 5, "replyOf"),                            # M2 replies to M1
        (2, 5, "likes"), (3, 5, "likes"),             # M1: 2 likes
        (4, 6, "likes"),                              # M2: 1 like
    ]
    for u, v, rel in edges:
        b.add_relationship(u, v, rel)
    g = b.finalize()

    assert q5.q5_active_posters(g, "T") == [(1, 2, 1, 3, 34)]


def test_q6_authoritative():
    # P1's tagged message M1 is liked by L1 and L2. L1's own message has 2 likes,
    # L2's own message has 1 like -> P1 score = 2 + 1 = 3.
    b = GraphSnapshotBuilder()
    nodes = [
        (0, "Tag"), (1, "Person"), (2, "Person"), (3, "Person"),
        (5, "Post"), (6, "Post"), (7, "Post"),
    ]
    for nid, label in nodes:
        b.add_node([label], node_id=nid)
    b.set_prop(0, "name", "T")
    for nid, ext in [(1, 1), (2, 2), (3, 3)]:
        b.set_prop(nid, "id", ext)
    edges = [
        (5, 0, "hasTag"), (1, 5, "hasCreator"),       # P1 created tagged M1
        (2, 5, "likes"), (3, 5, "likes"),             # L1, L2 liked M1
        (2, 6, "hasCreator"), (3, 7, "hasCreator"),   # L1 created M_a, L2 created M_b
        (1, 6, "likes"), (3, 6, "likes"),             # M_a: 2 likes
        (1, 7, "likes"),                              # M_b: 1 like
    ]
    for u, v, rel in edges:
        b.add_relationship(u, v, rel)
    g = b.finalize()

    assert q6.q6_authoritative(g, "T") == [(1, 3)]


def test_q7_related_topics():
    # Comments reply to M1 (tagged T). C1 carries U,V; C3 carries U; C2 carries U,T
    # (so C2 is skipped — it carries the target). Distinct comments per other tag:
    # U -> {C1, C3} = 2, V -> {C1} = 1.
    b = GraphSnapshotBuilder()
    nodes = [
        (0, "Tag"), (1, "Tag"), (2, "Tag"),
        (5, "Post"), (6, "Comment"), (7, "Comment"), (8, "Comment"),
    ]
    for nid, label in nodes:
        b.add_node([label], node_id=nid)
    b.set_prop(0, "name", "T")
    b.set_prop(1, "name", "U")
    b.set_prop(2, "name", "V")
    edges = [
        (5, 0, "hasTag"),
        (6, 5, "replyOf"), (6, 1, "hasTag"), (6, 2, "hasTag"),  # C1 -> U, V
        (7, 5, "replyOf"), (7, 1, "hasTag"), (7, 0, "hasTag"),  # C2 -> U, T (skipped)
        (8, 5, "replyOf"), (8, 1, "hasTag"),                    # C3 -> U
    ]
    for u, v, rel in edges:
        b.add_relationship(u, v, rel)
    g = b.finalize()

    assert q7.q7_related_topics(g, "T") == [("U", 2), ("V", 1)]


def test_q8_central_person():
    # P1 interested in T (base 100); P2 made one in-window tagged message (base 1).
    # P1<->P2 are friends, so friendsScore(P1)=1, friendsScore(P2)=100; both total
    # 101, so the tie breaks by id ascending.
    b = GraphSnapshotBuilder()
    nodes = [(0, "Tag"), (1, "Person"), (2, "Person"), (3, "Person"), (5, "Post")]
    for nid, label in nodes:
        b.add_node([label], node_id=nid)
    b.set_prop(0, "name", "T")
    for nid, ext in [(1, 1), (2, 2), (3, 3)]:
        b.set_prop(nid, "id", ext)
    b.set_prop(5, "day", 150)
    edges = [
        (1, 0, "hasInterest"),                    # P1 interested in T
        (5, 0, "hasTag"), (2, 5, "hasCreator"),   # P2's tagged message
        (1, 2, "knows"), (2, 1, "knows"),         # P1 <-> P2
    ]
    for u, v, rel in edges:
        b.add_relationship(u, v, rel)
    g = b.finalize()

    assert q8.q8_central_person(g, "T", 100, 200) == [(1, 100, 1), (2, 1, 100)]


def test_q9_thread_initiators():
    # Window [100,200]. P1's Post A (150): tree A,C1(160),C3(120) counted, C2(250)
    # pruned -> 3 msgs; P1's Post B(250) is out of window. P2's Post D(110): tree
    # D, C5(130) counted, C4(90) before window (not counted but traversed) -> 2.
    b = GraphSnapshotBuilder()
    nodes = [
        (1, "Person"), (2, "Person"),
        (5, "Post"), (6, "Comment"), (7, "Comment"), (8, "Comment"),
        (9, "Post"), (10, "Post"), (11, "Comment"), (12, "Comment"),
    ]
    for nid, label in nodes:
        b.add_node([label], node_id=nid)
    b.set_prop(1, "id", 1)
    b.set_prop(2, "id", 2)
    for nid, day in [(5, 150), (6, 160), (7, 250), (8, 120), (9, 250), (10, 110), (11, 90), (12, 130)]:
        b.set_prop(nid, "day", day)
    edges = [
        (1, 5, "hasCreator"), (1, 9, "hasCreator"), (2, 10, "hasCreator"),
        (6, 5, "replyOf"), (7, 6, "replyOf"), (8, 5, "replyOf"),  # A's tree
        (11, 10, "replyOf"), (12, 11, "replyOf"),                 # D's tree
    ]
    for u, v, rel in edges:
        b.add_relationship(u, v, rel)
    g = b.finalize()

    assert q9.q9_thread_initiators(g, 100, 200) == [(1, 1, 3), (2, 1, 2)]

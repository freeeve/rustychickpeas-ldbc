"""Tests for the Python LDBC BI loader + queries on tiny synthetic data."""

import gzip
import os

import props
import loader
from bi import q1, q3, q4, q5, q6, q7, q8, q9, q10, q11, q12, q13, q14, q15, q16, q17, q18, q19, q20
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
    rels = [
        (2, 0, "isPartOf"), (3, 1, "isPartOf"),
        (4, 2, "isLocatedIn"), (5, 2, "isLocatedIn"),
        (6, 3, "isLocatedIn"), (7, 3, "isLocatedIn"),
        (8, 4, "hasMember"), (8, 5, "hasMember"), (8, 6, "hasMember"),
        (9, 7, "hasMember"),  # F2's only member (forum excluded)
        (8, 10, "containerOf"),
        (11, 10, "replyOf"), (12, 11, "replyOf"),
        (6, 10, "hasCreator"), (4, 11, "hasCreator"), (5, 12, "hasCreator"),
    ]
    for u, v, rel in rels:
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
    rels = [
        (5, 0, "hasTag"), (6, 0, "hasTag"),
        (1, 5, "hasCreator"), (1, 6, "hasCreator"),  # P1 created both
        (6, 5, "replyOf"),                            # M2 replies to M1
        (2, 5, "likes"), (3, 5, "likes"),             # M1: 2 likes
        (4, 6, "likes"),                              # M2: 1 like
    ]
    for u, v, rel in rels:
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
    rels = [
        (5, 0, "hasTag"), (1, 5, "hasCreator"),       # P1 created tagged M1
        (2, 5, "likes"), (3, 5, "likes"),             # L1, L2 liked M1
        (2, 6, "hasCreator"), (3, 7, "hasCreator"),   # L1 created M_a, L2 created M_b
        (1, 6, "likes"), (3, 6, "likes"),             # M_a: 2 likes
        (1, 7, "likes"),                              # M_b: 1 like
    ]
    for u, v, rel in rels:
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
    rels = [
        (5, 0, "hasTag"),
        (6, 5, "replyOf"), (6, 1, "hasTag"), (6, 2, "hasTag"),  # C1 -> U, V
        (7, 5, "replyOf"), (7, 1, "hasTag"), (7, 0, "hasTag"),  # C2 -> U, T (skipped)
        (8, 5, "replyOf"), (8, 1, "hasTag"),                    # C3 -> U
    ]
    for u, v, rel in rels:
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
    rels = [
        (1, 0, "hasInterest"),                    # P1 interested in T
        (5, 0, "hasTag"), (2, 5, "hasCreator"),   # P2's tagged message
        (1, 2, "knows"), (2, 1, "knows"),         # P1 <-> P2
    ]
    for u, v, rel in rels:
        b.add_relationship(u, v, rel)
    g = b.finalize()

    assert q8.q8_central_person(g, "T", 100, 200) == [(1, 100, 1), (2, 1, 100)]


def test_q9_thread_initiators():
    # Window [100,200]. P1's Post A (150): tree A,C1(160),C3(120) counted, C2(250)
    # pruned -> 3 msgs; P1's Post B(250) is out of window. P2's Post D(110): tree
    # D, C5(130) counted, C4(90) before window (not counted but traversed) -> 2.
    # Contiguous node ids (the real loader assigns 0-based contiguous ids, which
    # roots_via/bfs_distances index by): P1=0, P2=1, A=2, C1=3, C2=4, C3=5, B=6,
    # D=7, C4=8, C5=9.
    b = GraphSnapshotBuilder()
    nodes = [
        (0, "Person"), (1, "Person"),
        (2, "Post"), (3, "Comment"), (4, "Comment"), (5, "Comment"),
        (6, "Post"), (7, "Post"), (8, "Comment"), (9, "Comment"),
    ]
    for nid, label in nodes:
        b.add_node([label], node_id=nid)
    b.set_prop(0, "id", 1)
    b.set_prop(1, "id", 2)
    for nid, day in [(2, 150), (3, 160), (4, 250), (5, 120), (6, 250), (7, 110), (8, 90), (9, 130)]:
        b.set_prop(nid, "day", day)
    rels = [
        (0, 2, "hasCreator"), (0, 6, "hasCreator"), (1, 7, "hasCreator"),
        (3, 2, "replyOf"), (4, 3, "replyOf"), (5, 2, "replyOf"),  # A's tree
        (8, 7, "replyOf"), (9, 8, "replyOf"),                     # D's tree
    ]
    for u, v, rel in rels:
        b.add_relationship(u, v, rel)
    g = b.finalize()

    assert q9.q9_thread_initiators(g, 100, 200) == [(1, 1, 3), (2, 1, 2)]


def test_q10_experts():
    # S(id10) knows A(id1) at dist1, A knows B(id2) at dist2. A and B live in
    # country X and each made a message tagged with Tg (class TC). Experts in
    # [1,2] hops -> A, B; one tagged message each.
    b = GraphSnapshotBuilder()
    nodes = [
        (0, "Person"), (1, "Person"), (2, "Person"),
        (3, "Country"), (4, "City"), (5, "TagClass"), (6, "Tag"),
        (7, "Post"), (8, "Post"),
    ]
    for nid, label in nodes:
        b.add_node([label], node_id=nid)
    b.set_prop(0, "id", 10)
    b.set_prop(1, "id", 1)
    b.set_prop(2, "id", 2)
    b.set_prop(3, "name", "X")
    b.set_prop(5, "name", "TC")
    b.set_prop(6, "name", "Tg")
    rels = [
        (0, 1, "knows"), (1, 0, "knows"), (1, 2, "knows"), (2, 1, "knows"),
        (4, 3, "isPartOf"), (1, 4, "isLocatedIn"), (2, 4, "isLocatedIn"),
        (6, 5, "hasType"),
        (1, 7, "hasCreator"), (2, 8, "hasCreator"),
        (7, 6, "hasTag"), (8, 6, "hasTag"),
    ]
    for u, v, rel in rels:
        b.add_relationship(u, v, rel)
    g = b.finalize()

    assert q10.q10_experts(g, 10, "X", "TC", 1, 2) == [(1, "Tg", 1), (2, "Tg", 1)]


def test_q11_friend_triangles():
    # A,B,C,E in country X; D out of country. In-window rels form triangles ABC
    # and ABE; C-E is out of window (250), so ACE/BCE don't count -> 2 triangles.
    b = GraphSnapshotBuilder()
    nodes = [(0, "Country"), (1, "City"), (2, "Person"), (3, "Person"),
             (4, "Person"), (5, "Person"), (6, "Person")]
    for nid, label in nodes:
        b.add_node([label], node_id=nid)
    b.set_prop(0, "name", "X")
    b.add_relationship(1, 0, "isPartOf")
    for p in (2, 3, 4, 6):  # A,B,C,E in country; D(5) is not located in the city
        b.add_relationship(p, 1, "isLocatedIn")

    def knows(u, v, kd):
        for s, t in ((u, v), (v, u)):
            b.add_relationship(s, t, "knows")
            b.set_relationship_prop_i64(s, t, "knows", "kd", kd)

    knows(2, 3, 150)  # A-B
    knows(3, 4, 160)  # B-C
    knows(2, 4, 170)  # A-C  -> triangle ABC
    knows(2, 6, 150)  # A-E
    knows(3, 6, 160)  # B-E  -> triangle ABE
    knows(4, 6, 250)  # C-E out of window (blocks ACE/BCE)
    knows(2, 5, 150)  # A-D, D out of country (ignored)
    g = b.finalize()

    assert q11.q11_friend_triangles(g, "X", 100, 200) == 2


def test_q12_message_counts():
    # Window day>100, len<20, content, root-lang in {en}. A creates P1(en) + C1
    # (reply to P1) -> 2 qualifying. P2(fr) and its reply, plus out-of-filter posts,
    # don't count. B qualifies for nothing. Histogram: one person with 2, one with 0.
    b = GraphSnapshotBuilder()
    nodes = [
        (0, "Post"), (1, "Post"), (2, "Comment"), (3, "Comment"),
        (4, "Post"), (5, "Post"), (6, "Post"), (7, "Person"), (8, "Person"),
    ]
    for nid, label in nodes:
        b.add_node([label], node_id=nid)
    # (day, content, len, lang) per message
    msgs = {
        0: (150, 1, 10, "en"), 1: (150, 1, 10, "fr"),
        2: (160, 1, 5, None), 3: (160, 1, 5, None),
        4: (50, 1, 10, "en"),    # before window
        5: (150, 0, 10, "en"),   # no content
        6: (150, 1, 25, "en"),   # too long
    }
    for nid, (day, content, length, lang) in msgs.items():
        b.set_prop(nid, "day", day)
        b.set_prop(nid, "content", content)
        b.set_prop(nid, "len", length)
        if lang is not None:
            b.set_prop(nid, "lang", lang)
    rels = [
        (2, 0, "replyOf"), (3, 1, "replyOf"),
        (7, 0, "hasCreator"), (7, 2, "hasCreator"),   # A created P1, C1
        (8, 1, "hasCreator"), (8, 3, "hasCreator"),   # B created P2, C2
        (8, 4, "hasCreator"), (8, 5, "hasCreator"), (8, 6, "hasCreator"),
    ]
    for u, v, rel in rels:
        b.add_relationship(u, v, rel)
    g = b.finalize()

    assert q12.q12_message_counts(g, 100, 20, ["en"]) == [(2, 1), (0, 1)]


def test_q13_zombies():
    # end_day=1000, end_ym=24. Z1,Z2 created before end_day with few messages ->
    # zombies; NZ has 5 messages (>= 2 months) -> not; Late created after end_day.
    # Z1's message is liked by Z2 (zombie) and NZ -> zlc=1, tlc=2 (score .5); Z2 has
    # no messages -> (0,0).
    b = GraphSnapshotBuilder()
    nodes = [(0, "Country"), (1, "City"), (2, "Person"), (3, "Person"),
             (4, "Person"), (5, "Person"), (6, "Post")]
    nodes += [(n, "Post") for n in range(7, 12)]  # NZ's 5 messages
    for nid, label in nodes:
        b.add_node([label], node_id=nid)
    b.set_prop(0, "name", "X")
    for nid, ext, pday, pym in [(2, 1, 500, 12), (3, 2, 500, 12), (4, 3, 500, 23), (5, 4, 1500, 12)]:
        b.set_prop(nid, "id", ext)
        b.set_prop(nid, "pday", pday)
        b.set_prop(nid, "pym", pym)
    for nid in [6] + list(range(7, 12)):
        b.set_prop(nid, "day", 600)
    rels = [
        (1, 0, "isPartOf"),
        (2, 1, "isLocatedIn"), (3, 1, "isLocatedIn"), (4, 1, "isLocatedIn"), (5, 1, "isLocatedIn"),
        (2, 6, "hasCreator"),                       # Z1 created M_z1
        (3, 6, "likes"), (4, 6, "likes"),           # Z2 (zombie) + NZ like M_z1
    ]
    rels += [(4, n, "hasCreator") for n in range(7, 12)]  # NZ created 5 messages
    for u, v, rel in rels:
        b.add_relationship(u, v, rel)
    g = b.finalize()

    assert q13.q13_zombies(g, "X", 1000, 24) == [(1, 1, 2), (2, 0, 0)]


def test_q14_international_dialog():
    # p1 (CityA, country C1) knows p2 (country C2). p1 replied to p2 (+4) and likes
    # p2's message (+10) -> score 14. p2 didn't reciprocate.
    b = GraphSnapshotBuilder()
    nodes = [(0, "Country"), (1, "Country"), (2, "City"), (3, "City"),
             (4, "Person"), (5, "Person"), (6, "Post"), (7, "Comment")]
    for nid, label in nodes:
        b.add_node([label], node_id=nid)
    b.set_prop(0, "name", "C1")
    b.set_prop(1, "name", "C2")
    b.set_prop(2, "name", "CityA")
    b.set_prop(3, "name", "CityB")
    b.set_prop(4, "id", 1)
    b.set_prop(5, "id", 2)
    rels = [
        (2, 0, "isPartOf"), (3, 1, "isPartOf"),
        (4, 2, "isLocatedIn"), (5, 3, "isLocatedIn"),
        (4, 5, "knows"), (5, 4, "knows"),
        (5, 6, "hasCreator"),               # p2 created M2
        (4, 7, "hasCreator"), (7, 6, "replyOf"),  # p1's comment replies to M2 (+4)
        (4, 6, "likes"),                    # p1 likes M2 (+10)
    ]
    for u, v, rel in rels:
        b.add_relationship(u, v, rel)
    g = b.finalize()

    assert q14.q14_international_dialog(g, "C1", "C2") == [(1, 2, "CityA", 14)]


def test_q16_fake_news():
    # Tag A on day 100: P1(x2),P2,P3,P4,P5 all posted. max_knows=1. P3 knows P4 and
    # P5 (both same-day posters) -> excluded; P1,P2,P4,P5 pass. Tag B on day 200:
    # only P1. Intersection -> P1 with (countA=2, countB=1).
    b = GraphSnapshotBuilder()
    nodes = [(0, "Tag"), (1, "Tag")]
    nodes += [(n, "Person") for n in range(2, 7)]       # P1..P5 = nodes 2..6
    nodes += [(n, "Post") for n in range(7, 14)]        # 7..12 tag-A, 13 tag-B
    for nid, label in nodes:
        b.add_node([label], node_id=nid)
    b.set_prop(0, "name", "A")
    b.set_prop(1, "name", "B")
    for nid, ext in [(2, 1), (3, 2), (4, 3), (5, 4), (6, 5)]:
        b.set_prop(nid, "id", ext)
    for nid in range(7, 13):
        b.set_prop(nid, "day", 100)
    b.set_prop(13, "day", 200)
    rels = [
        (7, 0, "hasTag"), (8, 0, "hasTag"), (9, 0, "hasTag"),
        (10, 0, "hasTag"), (11, 0, "hasTag"), (12, 0, "hasTag"), (13, 1, "hasTag"),
        (2, 7, "hasCreator"), (2, 8, "hasCreator"),  # P1 made two tag-A messages
        (3, 9, "hasCreator"), (4, 10, "hasCreator"),
        (5, 11, "hasCreator"), (6, 12, "hasCreator"), (2, 13, "hasCreator"),
        (2, 3, "knows"), (3, 2, "knows"),            # P1-P2
        (4, 5, "knows"), (5, 4, "knows"),            # P3-P4
        (4, 6, "knows"), (6, 4, "knows"),            # P3-P5
    ]
    for u, v, rel in rels:
        b.add_relationship(u, v, rel)
    g = b.finalize()

    ra = q16.q16_param_result(g, "A", 100, 1)
    assert sorted((g.get_property(p, "id"), c) for p, c in ra.items()) == [(1, 2), (2, 1), (4, 1), (5, 1)]
    rb = q16.q16_param_result(g, "B", 200, 1)
    assert q16.q16_fake_news(g, ra, rb) == [(1, 2, 1)]


def test_q18_friend_recommendation():
    # P1,P2,P3 interested in T. M1,M2 are mutual friends. P1-P2 share {M1,M2} (=2);
    # P2-P3 share {M1} (=1). P1-P3 are directly known, so that pair is excluded.
    b = GraphSnapshotBuilder()
    nodes = [(0, "Tag"), (1, "Person"), (2, "Person"), (3, "Person"),
             (4, "Person"), (5, "Person")]
    for nid, label in nodes:
        b.add_node([label], node_id=nid)
    b.set_prop(0, "name", "T")
    for nid, ext in [(1, 1), (2, 2), (3, 3)]:
        b.set_prop(nid, "id", ext)

    def knows(u, v):
        b.add_relationship(u, v, "knows")
        b.add_relationship(v, u, "knows")

    for p in (1, 2, 3):
        b.add_relationship(p, 0, "hasInterest")
    knows(1, 4)  # P1-M1
    knows(1, 5)  # P1-M2
    knows(4, 2)  # M1-P2
    knows(5, 2)  # M2-P2
    knows(4, 3)  # M1-P3
    knows(1, 3)  # P1-P3 directly known
    g = b.finalize()

    assert q18.q18_friend_recommendation(g, "T") == [(1, 2, 2), (2, 1, 2), (2, 3, 1), (3, 2, 1)]


def test_q19_interaction_path():
    # p1(City1) -knows- x -knows- p2(City2). p1<->x interacted twice (weight 0.5),
    # x<->p2 once (weight 1.0). Shortest p1->p2 = 0.5 + 1.0 = 1.5.
    b = GraphSnapshotBuilder()
    nodes = [(0, "City"), (1, "City"), (2, "Person"), (3, "Person"), (4, "Person"),
             (5, "Post"), (6, "Post"), (7, "Comment"), (8, "Comment"), (9, "Comment")]
    for nid, label in nodes:
        b.add_node([label], node_id=nid)
    b.set_prop(0, "id", 100)
    b.set_prop(1, "id", 200)
    b.set_prop(2, "id", 1)
    b.set_prop(3, "id", 2)
    b.set_prop(4, "id", 3)
    rels = [
        (2, 0, "isLocatedIn"), (3, 1, "isLocatedIn"),
        (2, 4, "knows"), (4, 2, "knows"), (4, 3, "knows"), (3, 4, "knows"),
        (4, 5, "hasCreator"), (3, 6, "hasCreator"),   # x made Mx, p2 made Mp2
        (2, 7, "hasCreator"), (2, 8, "hasCreator"), (4, 9, "hasCreator"),
        (7, 5, "replyOf"), (8, 5, "replyOf"),         # p1 replied to x twice
        (9, 6, "replyOf"),                            # x replied to p2 once
    ]
    for u, v, rel in rels:
        b.add_relationship(u, v, rel)
    g = b.finalize()

    interaction = q19.build_interaction_map(g)
    assert interaction.to_dict() == {(2, 4): 2, (3, 4): 1}
    assert q19.q19_interaction_path(g, 100, 200, interaction) == [(1, 2, 1.5)]


def test_q20_recruitment():
    # e1,e2 work at Co. Both and target p2 studied at Uni: e1@2010 vs p2@2012 -> diff
    # 2 -> weight 3; e2@2007 vs p2@2012 -> diff 5 -> weight 6. Direct knows rels, so
    # e1 reaches p2 at 3.0, e2 at 6.0.
    b = GraphSnapshotBuilder()
    nodes = [(0, "Company"), (1, "University"), (2, "Person"), (3, "Person"), (4, "Person")]
    for nid, label in nodes:
        b.add_node([label], node_id=nid)
    b.set_prop(0, "name", "Co")
    b.set_prop(2, "id", 1)
    b.set_prop(3, "id", 2)
    b.set_prop(4, "id", 66)
    for u, v, rel in [(2, 0, "workAt"), (3, 0, "workAt"),
                      (2, 4, "knows"), (4, 2, "knows"), (3, 4, "knows"), (4, 3, "knows")]:
        b.add_relationship(u, v, rel)
    for person, year in [(2, 2010), (3, 2007), (4, 2012)]:
        b.add_relationship(person, 1, "studyAt")
        b.set_relationship_prop_i64(person, 1, "studyAt", "cy", year)
    g = b.finalize()

    studyat = q20.build_studyat(g)
    wm = q20.build_study_weight_map(g, studyat)
    assert q20.q20_recruitment(g, "Co", 66, wm) == [(1, 3.0), (2, 6.0)]


def test_q15_weighted_path():
    # p1(id14) knows p2(id16). p1's comment replies to p2's Post; that post's forum
    # was created in-window -> interaction weight 1.0 (a Post). Rel weight = 1/(1+1)
    # = 0.5, so the p1->p2 path cost is 0.5.
    b = GraphSnapshotBuilder()
    nodes = [(0, "Forum"), (1, "Person"), (2, "Person"), (3, "Post"), (4, "Comment")]
    for nid, label in nodes:
        b.add_node([label], node_id=nid)
    b.set_prop(0, "fday", 150)
    b.set_prop(1, "id", 14)
    b.set_prop(2, "id", 16)
    rels = [
        (1, 2, "knows"), (2, 1, "knows"),
        (0, 3, "containerOf"),               # forum contains the post
        (2, 3, "hasCreator"),                # p2 made the post
        (1, 4, "hasCreator"), (4, 3, "replyOf"),  # p1's comment replies to it
    ]
    for u, v, rel in rels:
        b.add_relationship(u, v, rel)
    g = b.finalize()

    assert q15.q15_weighted_path(g, 14, 16, 100, 200) == 0.5
    # Unreachable target -> -1.
    assert q15.q15_weighted_path(g, 14, 999, 100, 200) == -1.0


def test_q17_information_propagation():
    # m1 (by p1, in F1, tagged) and msg2 (by p3, in F2, tagged); p2's tagged comment
    # replies to msg2. p2,p3 are F1 members; p1 is not an F2 member; msg2 is >1h after
    # m1. So p1 "received" msg2 -> one message.
    b = GraphSnapshotBuilder()
    nodes = [(0, "Tag"), (1, "Forum"), (2, "Forum"), (3, "Person"), (4, "Person"),
             (5, "Person"), (6, "Post"), (7, "Post"), (8, "Comment")]
    for nid, label in nodes:
        b.add_node([label], node_id=nid)
    b.set_prop(0, "name", "T")
    b.set_prop(3, "id", 1)
    b.set_prop(4, "id", 2)
    b.set_prop(5, "id", 3)
    b.set_prop(6, "ms", 1000)
    b.set_prop(7, "ms", 10_000_000)
    b.set_prop(8, "ms", 10_000_001)
    rels = [
        (6, 0, "hasTag"), (7, 0, "hasTag"), (8, 0, "hasTag"),
        (1, 6, "containerOf"), (2, 7, "containerOf"),
        (3, 6, "hasCreator"), (5, 7, "hasCreator"), (4, 8, "hasCreator"),
        (8, 7, "replyOf"),
        (1, 4, "hasMember"), (1, 5, "hasMember"),  # p2, p3 are F1 members
    ]
    for u, v, rel in rels:
        b.add_relationship(u, v, rel)
    g = b.finalize()

    assert q17.q17_information_propagation(g, "T", 1) == [(1, 1)]

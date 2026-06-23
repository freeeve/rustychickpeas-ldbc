"""Tests for the Python LDBC Interactive (IC/IS) queries on tiny synthetic data."""

from ic import (
    is1, is2, is3, is4, is5, is6, is7,
    ic1, ic2, ic3, ic4, ic5, ic6, ic7, ic8, ic9, ic10, ic11, ic12, ic13, ic14,
)
from rustychickpeas import Direction, GraphSnapshotBuilder

_NAMES = {0: ("Ann", "Alpha"), 1: ("Bob", "Beta"), 2: ("Ann", "Gamma")}


def _social():
    # Persons 0,1,2 with a knows path 0-1-2. Messages 3(Post by1), 4(Comment by2),
    # 5(Post by1); comment 4 replies post 3. day/ms ascending with id. Contiguous ids
    # (fold_via / neighbor_via / dijkstra / bfs_distances index by node id).
    b = GraphSnapshotBuilder()
    for nid in (0, 1, 2):
        b.add_node(["Person"], node_id=nid)
        b.set_prop(nid, "id", 100 + nid)
        b.set_prop(nid, "fname", _NAMES[nid][0])
        b.set_prop(nid, "lname", _NAMES[nid][1])
        b.set_prop(nid, "pday", nid)
    for nid, label in [(3, "Post"), (4, "Comment"), (5, "Post")]:
        b.add_node([label], node_id=nid)
        b.set_prop(nid, "day", 10 * (nid - 2))   # 10, 20, 30
        b.set_prop(nid, "ms", 1000 * (nid - 2))  # 1000, 2000, 3000
    for u, v in [(0, 1), (1, 2)]:
        b.add_relationship(u, v, "knows")
        b.add_relationship(v, u, "knows")
    for creator, msg in [(1, 3), (2, 4), (1, 5)]:
        b.add_relationship(creator, msg, "hasCreator")
    b.add_relationship(4, 3, "replyOf")  # comment 4 -> post 3
    return b.finalize()


def test_is1_profile():
    g = _social()
    assert is1.is1_profile(g, 1) == ("Bob", "Beta", 1)


def test_ic1_friends_by_name():
    g = _social()
    # Within 3 knows hops of person 0 with first name "Ann": only person 2 (dist 2);
    # person 0 itself is excluded (dist 0).
    assert ic1.ic1_friends_by_name(g, 0, "Ann") == [(2, 2, "Gamma")]


def test_is2_recent_of_person():
    g = _social()
    assert is2.is2_recent_of_person(g, 1, 100) == [(5, 3000), (3, 1000)]


def test_is3_friends():
    g = _social()
    assert sorted(is3.is3_friends(g, 0)) == [1]
    assert sorted(is3.is3_friends(g, 1)) == [0, 2]


def test_is5_message_creator():
    g = _social()
    assert is5.is5_message_creator(g, 3) == 1
    assert is5.is5_message_creator(g, 4) == 2


def test_ic2_recent_messages():
    g = _social()
    # friends(0) = {1}; 1's messages 5,3 by ms desc.
    assert ic2.ic2_recent_messages(g, 0, 100) == [(5, 3000), (3, 1000)]


def test_ic9_fof_messages():
    g = _social()
    # FoF(0) within 1..2 hops = {1, 2}; messages 5,4,3 by ms desc.
    assert ic9.ic9_fof_messages(g, 0, 100) == [(5, 3000), (4, 2000), (3, 1000)]


def test_ic13_shortest_path():
    g = _social()
    assert ic13.ic13_shortest_path(g, 0, 2) == 2   # 0-1-2
    assert ic13.ic13_shortest_path(g, 0, 0) == 0


def test_ic14_weighted_path():
    g = _social()
    interaction = ic14.build_interaction(g)
    # one reply interaction between creators 1 and 2 -> rel (1,2) cost 1/(1+1)=0.5;
    # rel (0,1) has no interaction -> 1/(0+1)=1.0. Path 0-1-2 = 1.5.
    assert ic14.ic14_weighted_path(g, 0, 2, interaction) == 1.5


def _thread():
    # Person 0(moderator), 1(author), 2(replier). Post 3 by 1; Comment 4 replies 3,
    # by 2. Forum 5 containerOf Post 3, hasModerator 0. Author 1 knows replier 2.
    b = GraphSnapshotBuilder()
    for nid in (0, 1, 2):
        b.add_node(["Person"], node_id=nid)
        b.set_prop(nid, "id", 100 + nid)
    for nid, label, ms in [(3, "Post", 1000), (4, "Comment", 2000)]:
        b.add_node([label], node_id=nid)
        b.set_prop(nid, "ms", ms)
    b.add_node(["Forum"], node_id=5)
    b.set_prop(5, "id", 500)
    b.add_relationship(1, 3, "hasCreator")
    b.add_relationship(2, 4, "hasCreator")
    b.add_relationship(4, 3, "replyOf")
    b.add_relationship(5, 3, "containerOf")
    b.add_relationship(5, 0, "hasModerator")
    b.add_relationship(1, 2, "knows")
    b.add_relationship(2, 1, "knows")
    return b.finalize()


def test_ic8_recent_replies():
    g = _thread()
    assert ic8.ic8_recent_replies(g, 1) == [(4, 2000)]  # reply 4 to person 1's post 3


def test_is6_forum_of_message():
    g = _thread()
    roots = g.roots_via("replyOf", Direction.Outgoing)
    assert is6.is6_forum_of_message(g, 4, roots) == (5, 0)  # root post 3 -> forum 5, mod 0
    assert is6.is6_forum_of_message(g, 3, roots) == (5, 0)


def test_is7_replies():
    g = _thread()
    # reply 4 (ms 2000) by person 2, who is a knows-friend of post 3's author (1).
    assert is7.is7_replies(g, 3) == [(4, 2000, 2, True)]


def _tags():
    # Persons 0,1,2 with knows 0-1-2. Posts 3(by 1), 4(by 2). Tags 5("T" target),
    # 6("Apple"), 7("Banana"). Both posts carry T; 3 also Apple, 4 also Banana.
    b = GraphSnapshotBuilder()
    for nid in (0, 1, 2):
        b.add_node(["Person"], node_id=nid)
    for nid in (3, 4):
        b.add_node(["Post"], node_id=nid)
    for nid, name in [(5, "T"), (6, "Apple"), (7, "Banana")]:
        b.add_node(["Tag"], node_id=nid)
        b.set_prop(nid, "name", name)
    for u, v in [(0, 1), (1, 2)]:
        b.add_relationship(u, v, "knows")
        b.add_relationship(v, u, "knows")
    b.add_relationship(1, 3, "hasCreator")
    b.add_relationship(2, 4, "hasCreator")
    for post, tag in [(3, 5), (3, 6), (4, 5), (4, 7)]:
        b.add_relationship(post, tag, "hasTag")
    return b.finalize()


def test_ic6_tag_cooccurrence():
    g = _tags()
    # FoF(0) = {1, 2}; their posts 3, 4 both carry T; co-occurring Apple(1), Banana(1).
    assert ic6.ic6_tag_cooccurrence(g, 0, "T") == [(6, 1), (7, 1)]


def test_ic4_new_topics():
    # Seed 0 knows friend 1. Friend's Post 2 (day 5, tag A) is before window [10,20);
    # Post 3 (day 15, tag B) and Post 4 (day 16, tags B,C) are in-window. A is excluded
    # (seen before); B count 2, C count 1.
    b = GraphSnapshotBuilder()
    for nid in (0, 1):
        b.add_node(["Person"], node_id=nid)
    for nid, day in [(2, 5), (3, 15), (4, 16)]:
        b.add_node(["Post"], node_id=nid)
        b.set_prop(nid, "day", day)
    for nid, name in [(5, "A"), (6, "B"), (7, "C")]:
        b.add_node(["Tag"], node_id=nid)
        b.set_prop(nid, "name", name)
    b.add_relationship(0, 1, "knows"); b.add_relationship(1, 0, "knows")
    for p in (2, 3, 4):
        b.add_relationship(1, p, "hasCreator")
    for post, tag in [(2, 5), (3, 6), (4, 6), (4, 7)]:
        b.add_relationship(post, tag, "hasTag")
    g = b.finalize()
    assert ic4.ic4_new_topics(g, 0, 10, 10) == [(6, 2), (7, 1)]


def test_ic3_two_country():
    # Seed 0 knows friend 1 (home Country Z, not X/Y). Friend posted in X (day 15) and
    # Y (day 16), both in window [10,20).
    b = GraphSnapshotBuilder()
    for nid in (0, 1):
        b.add_node(["Person"], node_id=nid)
        b.set_prop(nid, "id", 100 + nid)
    for nid, name in [(2, "X"), (3, "Y"), (4, "Z")]:
        b.add_node(["Country"], node_id=nid)
        b.set_prop(nid, "name", name)
    b.add_node(["City"], node_id=5)
    for nid, day in [(6, 15), (7, 16)]:
        b.add_node(["Comment"], node_id=nid)
        b.set_prop(nid, "day", day)
    b.add_relationship(0, 1, "knows"); b.add_relationship(1, 0, "knows")
    b.add_relationship(1, 5, "isLocatedIn")
    b.add_relationship(5, 4, "isPartOf")
    b.add_relationship(1, 6, "hasCreator"); b.add_relationship(1, 7, "hasCreator")
    b.add_relationship(6, 2, "msgCountry")
    b.add_relationship(7, 3, "msgCountry")
    g = b.finalize()
    assert ic3.ic3_friends_two_countries(g, 0, "X", "Y", 10, 10) == [(1, 1, 1)]


def test_ic12_expert_search():
    # Seed 0 knows friend 1. Post 2 tagged T; T hasType class C. Friend's Comment 3
    # replies Post 2 -> friend 1 is an expert with 1 qualifying reply.
    b = GraphSnapshotBuilder()
    for nid in (0, 1):
        b.add_node(["Person"], node_id=nid)
        b.set_prop(nid, "id", 100 + nid)
    b.add_node(["Post"], node_id=2)
    b.add_node(["Comment"], node_id=3)
    b.add_node(["Tag"], node_id=4); b.set_prop(4, "name", "T")
    b.add_node(["TagClass"], node_id=5); b.set_prop(5, "name", "C")
    b.add_relationship(0, 1, "knows"); b.add_relationship(1, 0, "knows")
    b.add_relationship(1, 3, "hasCreator")
    b.add_relationship(3, 2, "replyOf")
    b.add_relationship(2, 4, "hasTag")
    b.add_relationship(4, 5, "hasType")
    g = b.finalize()
    assert ic12.ic12_expert_search(g, 0, "C") == [(1, 1, ["T"])]


def test_ic10_friend_recommend():
    # Seed 0 -knows- 1 -knows- 2 (foaf). 0 is interested in Tag T. foaf 2 born 01-21
    # (in window for month 1); its Posts 4,5 carry T (common), Post 6 carries U
    # (uncommon) -> score 2 - 1 = 1.
    b = GraphSnapshotBuilder()
    for nid in (0, 1, 2):
        b.add_node(["Person"], node_id=nid)
        b.set_prop(nid, "id", 100 + nid)
    b.set_prop(2, "bmon", 1)
    b.set_prop(2, "bdom", 21)
    b.add_node(["Tag"], node_id=3); b.set_prop(3, "name", "T")
    for nid in (4, 5, 6):
        b.add_node(["Post"], node_id=nid)
    b.add_node(["Tag"], node_id=7); b.set_prop(7, "name", "U")
    b.add_relationship(0, 1, "knows"); b.add_relationship(1, 0, "knows")
    b.add_relationship(1, 2, "knows"); b.add_relationship(2, 1, "knows")
    b.add_relationship(0, 3, "hasInterest")
    for p in (4, 5, 6):
        b.add_relationship(2, p, "hasCreator")
    b.add_relationship(4, 3, "hasTag"); b.add_relationship(5, 3, "hasTag")
    b.add_relationship(6, 7, "hasTag")
    g = b.finalize()
    assert ic10.ic10_friend_recommend(g, 0, 1) == [(2, 1)]


def test_ic11_job_referral():
    # Seed 0 -knows- 1; 1 worked at Company 2 (workFrom 2010), located in City 3 of
    # Country X. Referral for X with year cutoff 2030.
    b = GraphSnapshotBuilder()
    for nid in (0, 1):
        b.add_node(["Person"], node_id=nid)
        b.set_prop(nid, "id", 100 + nid)
    b.add_node(["Company"], node_id=2); b.set_prop(2, "name", "Acme")
    b.add_node(["City"], node_id=3)
    b.add_node(["Country"], node_id=4); b.set_prop(4, "name", "X")
    b.add_relationship(0, 1, "knows"); b.add_relationship(1, 0, "knows")
    b.add_relationship(3, 4, "isPartOf")
    b.add_relationship(2, 3, "orgPlace")
    b.add_relationship(1, 2, "workAt")
    b.set_relationship_prop(1, 2, "workAt", "wf", 2010)
    g = b.finalize()
    assert ic11.ic11_job_referral(g, 0, "X", 2030) == [(1, 2, 2010)]


def test_ic5_new_groups():
    # Seed 0 knows member 1, who joined Forum 2 on day 15 (> min_day 10). Forum 2
    # contains Post 3, created by member 1 -> forum 2 count 1.
    b = GraphSnapshotBuilder()
    for nid in (0, 1):
        b.add_node(["Person"], node_id=nid)
    b.add_node(["Forum"], node_id=2); b.set_prop(2, "id", 200)
    b.add_node(["Post"], node_id=3)
    b.add_relationship(0, 1, "knows"); b.add_relationship(1, 0, "knows")
    b.add_relationship(2, 1, "hasMember")
    b.set_relationship_prop(2, 1, "hasMember", "hd", 15)
    b.add_relationship(2, 3, "containerOf")
    b.add_relationship(1, 3, "hasCreator")
    g = b.finalize()
    assert ic5.ic5_new_groups(g, 0, 10) == [(2, 1)]


def test_ic7_recent_likers():
    # Seed 0's Post 2 is liked by friend 1 (ld 1000) and non-friend 3 (ld 2000).
    # Ordered by like time desc: non-friend 3 (is_new), then friend 1.
    b = GraphSnapshotBuilder()
    for nid in (0, 1, 3):
        b.add_node(["Person"], node_id=nid)
        b.set_prop(nid, "id", 100 + nid)
    b.add_node(["Post"], node_id=2)
    b.add_relationship(0, 1, "knows"); b.add_relationship(1, 0, "knows")
    b.add_relationship(0, 2, "hasCreator")
    b.add_relationship(1, 2, "likes"); b.set_relationship_prop(1, 2, "likes", "ld", 1000)
    b.add_relationship(3, 2, "likes"); b.set_relationship_prop(3, 2, "likes", "ld", 2000)
    g = b.finalize()
    assert ic7.ic7_recent_likers(g, 0) == [(3, 2000, 2, True), (1, 1000, 2, False)]


def test_is4_message_content():
    b = GraphSnapshotBuilder()
    b.add_node(["Post"], node_id=0)
    b.set_prop(0, "ms", 12345)
    b.set_prop(0, "ctext", "hello world")
    b.add_node(["Comment"], node_id=1)  # no ctext -> None
    g = b.finalize()
    assert is4.is4_message_content(g, 0) == (12345, "hello world")
    assert is4.is4_message_content(g, 1) is None

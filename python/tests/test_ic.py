"""Tests for the Python LDBC Interactive (IC/IS) queries on tiny synthetic data."""

from ic import is2, is3, is5, ic2, ic9, ic13, ic14
from rustychickpeas import GraphSnapshotBuilder


def _social():
    # Persons 0,1,2 with a knows path 0-1-2. Messages 3(Post by1), 4(Comment by2),
    # 5(Post by1); comment 4 replies post 3. day/ms ascending with id. Contiguous ids
    # (fold_via / neighbor_via / dijkstra / bfs_distances index by node id).
    b = GraphSnapshotBuilder()
    for nid in (0, 1, 2):
        b.add_node(["Person"], node_id=nid)
        b.set_prop(nid, "id", 100 + nid)
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
    # one reply interaction between creators 1 and 2 -> edge (1,2) cost 1/(1+1)=0.5;
    # edge (0,1) has no interaction -> 1/(0+1)=1.0. Path 0-1-2 = 1.5.
    assert ic14.ic14_weighted_path(g, 0, 2, interaction) == 1.5

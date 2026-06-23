"""Unit tests for the Python Graphalytics algorithms, mirroring the Rust
src/graphalytics/mod.rs tests + the loader/validator seams."""

import math
import os
import sys

sys.path.insert(0, os.path.dirname(os.path.dirname(os.path.abspath(__file__))))

from graphalytics import algos, load, validate  # noqa: E402
from rustychickpeas import GraphSnapshotBuilder  # noqa: E402

INF = algos.BFS_UNREACHABLE


def _build(n, rels):
    b = GraphSnapshotBuilder(capacity_nodes=n + 1, capacity_rels=1)
    for i in range(n):
        b.add_node(["V"], node_id=i)
    for u, v in rels:
        b.add_relationship(u, v, "e")
    return b.finalize()


def _build_weighted(n, rels):
    b = GraphSnapshotBuilder(capacity_nodes=n + 1, capacity_rels=1)
    for i in range(n):
        b.add_node(["V"], node_id=i)
    for u, v, w in rels:
        b.add_relationship(u, v, "e")
        b.set_relationship_prop(u, v, "e", "weight", w)
    return b.finalize()


def test_bfs_depths_and_unreachable():
    g = _build(4, [(0, 1), (1, 2)])
    assert algos.bfs(g, 0, 4, True) == [0, 1, 2, INF]


def test_sssp_weighted_shortest_and_unreachable():
    g = _build_weighted(4, [(0, 1, 2.0), (1, 2, 3.0), (0, 2, 10.0)])
    d = algos.sssp(g, 0, 4, True, True)
    assert d[0] == 0.0 and d[1] == 2.0 and d[2] == 5.0
    assert math.isinf(d[3])


def test_wcc_two_components_label_min_id():
    g = _build(5, [(0, 1), (1, 2), (3, 4)])
    assert algos.wcc(g, 5) == [0, 0, 0, 3, 3]


def test_pagerank_uniform_on_directed_cycle():
    g = _build(3, [(0, 1), (1, 2), (2, 0)])
    pr = algos.pagerank(g, 3, True, 0.85, 30)
    assert all(abs(p - 1.0 / 3.0) < 1e-9 for p in pr)
    assert abs(sum(pr) - 1.0) < 1e-9


def test_pagerank_redistributes_sink_rank():
    g = _build(2, [(0, 1)])
    pr = algos.pagerank(g, 2, True, 0.85, 1)
    assert abs(pr[0] - 0.2875) < 1e-9
    assert abs(pr[1] - 0.7125) < 1e-9
    assert abs(sum(pr) - 1.0) < 1e-9


def test_cdlp_triangle_converges_to_min_label():
    g = _build(3, [(0, 1), (1, 2), (2, 0)])
    assert algos.cdlp(g, 3, False, 2, [0, 1, 2]) == [0, 0, 0]


def test_cdlp_seeded_runs_in_seed_label_space():
    g = _build(3, [(0, 1), (1, 2), (2, 0)])
    assert algos.cdlp(g, 3, False, 2, [10, 20, 30]) == [10, 10, 10]


def test_lcc_triangle_with_pendant():
    g = _build(4, [(0, 1), (1, 2), (2, 0), (0, 3)])
    c = algos.lcc(g, 4, False)
    assert abs(c[0] - 1.0 / 3.0) < 1e-9
    assert abs(c[1] - 1.0) < 1e-9 and abs(c[2] - 1.0) < 1e-9
    assert c[3] == 0.0


def test_lcc_gallop_branch_on_high_degree_neighbour():
    g = _build(5, [(0, 1), (0, 2), (1, 2), (1, 3), (1, 4)])
    c = algos.lcc(g, 5, True)
    assert abs(c[0] - 0.5) < 1e-9


def test_loader_maps_vertices_and_parses_params():
    ds = load._build("10\n20\n30\n", _e_file("10 20 2.5\n20 30 4.0\n10 30\n"),
                     PROPS, load._parse_params(PROPS))
    assert ds.vertex_of_node == [10, 20, 30]
    assert ds.node(10) == 0 and ds.node(30) == 2 and ds.node(99) is None
    assert len(ds) == 3
    assert not ds.params.directed
    assert ds.params.bfs_source == 20 and ds.params.sssp_source == 10
    assert ds.params.pr_iterations == 7 and ds.params.cdlp_iterations == 9


def test_validators():
    ds = load._build("1\n2\n3\n", _e_file(""), "", load.Params())
    assert validate.check_exact_i64(ds, [0, 1, 2], {1: "0", 2: "1", 3: "2"}) is None
    assert validate.check_exact_i64(ds, [0, 9, 2], {1: "0", 2: "1", 3: "2"}) is not None
    r = {1: "0.5", 2: "0.25", 3: "inf"}
    assert validate.check_epsilon(ds, [0.5 + 1e-9, 0.25, float("inf")], r) is None
    assert validate.check_epsilon(ds, [0.5, 0.25, 1.0], r) is not None
    same = {1: "100", 2: "100", 3: "200"}
    assert validate.check_relabel(ds, [5, 5, 7], same) is None
    reshaped = {1: "100", 2: "200", 3: "200"}
    assert validate.check_relabel(ds, [5, 5, 7], reshaped) is not None


PROPS = """\
graph.x.directed = false
graph.x.bfs.source-vertex = 20
graph.x.pr.damping-factor = 0.85
graph.x.pr.num-iterations = 7
graph.x.cdlp.max-iterations = 9
graph.x.sssp.source-vertex = 10
graph.x.edge-properties.names = weight
"""


def _e_file(text):
    """Write edge text to a temp file path the loader's _build streams from."""
    import tempfile

    fd, path = tempfile.mkstemp(suffix=".e")
    with os.fdopen(fd, "w") as fh:
        fh.write(text)
    return path

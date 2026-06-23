"""Graphalytics dataset loader: `<name>.v` / `<name>.e` / `<name>.properties` into
a rustychickpeas GraphSnapshot plus the dense-node <-> original-vertex id maps and
the per-algorithm run parameters (LDBC Graphalytics spec v1.0.x, mirrors
src/graphalytics/load.rs).

Vertices get dense node ids in `.v` order (node i == vertex_of_node[i]); each `.e`
line `src dst [weight]` becomes an ``e`` rel with an f64 ``weight`` prop (default
1.0). Undirected graphs store each rel once; algorithms traverse Direction.Both.
"""

import gc
import os

from rustychickpeas import GraphSnapshotBuilder


class Params:
    """Per-dataset algorithm parameters parsed from `<name>.properties` (sources are
    original vertex ids; resolve via Dataset.node)."""

    def __init__(self):
        self.directed = True
        self.bfs_source = None
        self.sssp_source = None
        self.pr_damping = 0.85
        self.pr_iterations = 10
        self.cdlp_iterations = 10
        self.weighted = False


class Dataset:
    def __init__(self, graph, params, vertex_of_node, node_of_vertex):
        self.graph = graph
        self.params = params
        self.vertex_of_node = vertex_of_node      # node id -> original vertex id
        self.node_of_vertex = node_of_vertex      # original vertex id -> dense node id

    def node(self, vertex):
        return self.node_of_vertex.get(vertex)

    def __len__(self):
        return len(self.vertex_of_node)


def _parse_params(props):
    p = Params()
    for line in props.splitlines():
        line = line.strip()
        if not line or line.startswith("#") or "=" not in line:
            continue
        key, val = line.split("=", 1)
        key, val = key.strip(), val.strip()
        if key.endswith(".directed"):
            p.directed = val.lower() == "true"
        elif key.endswith(".bfs.source-vertex"):
            p.bfs_source = _try_int(val)
        elif key.endswith(".sssp.source-vertex"):
            p.sssp_source = _try_int(val)
        elif key.endswith(".pr.damping-factor"):
            p.pr_damping = _try_float(val, p.pr_damping)
        elif key.endswith(".pr.num-iterations"):
            p.pr_iterations = _try_int(val) or p.pr_iterations
        elif key.endswith(".cdlp.max-iterations"):
            p.cdlp_iterations = _try_int(val) or p.cdlp_iterations
        elif key.endswith(".edge-properties.names"):
            p.weighted = "weight" in val
    return p


def _try_int(s):
    try:
        return int(s)
    except ValueError:
        return None


def _try_float(s, default):
    try:
        return float(s)
    except ValueError:
        return default


def load(directory, name):
    """Load `<directory>/<name>.{v,e,properties}` into a Dataset."""
    with open(os.path.join(directory, f"{name}.v"), encoding="utf-8") as fh:
        v_text = fh.read()
    e_path = os.path.join(directory, f"{name}.e")
    props_path = os.path.join(directory, f"{name}.properties")
    props = ""
    if os.path.exists(props_path):
        with open(props_path, encoding="utf-8") as fh:
            props = fh.read()
    params = _parse_params(props)

    gc.disable()  # bulk-allocate load; avoid GC churn on large datasets (e.g. wiki-Talk)
    try:
        return _build(v_text, e_path, props, params)
    finally:
        gc.enable()


def _build(v_text, e_path, props, params):
    vertex_of_node = []
    node_of_vertex = {}
    for line in v_text.splitlines():
        tok = line.split()
        if not tok:
            continue
        vid = _try_int(tok[0])
        if vid is not None and vid not in node_of_vertex:
            node_of_vertex[vid] = len(vertex_of_node)
            vertex_of_node.append(vid)

    n = len(vertex_of_node)
    b = GraphSnapshotBuilder(capacity_nodes=n + 1, capacity_rels=1)
    for i in range(n):
        b.add_node(["V"], node_id=i)

    # Stream the edge file; set the weight only when present (so an unweighted graph
    # like wiki-Talk skips the per-rel prop write entirely).
    with open(e_path, encoding="utf-8") as fh:
        for line in fh:
            parts = line.split()
            if len(parts) < 2:
                continue
            sv = _try_int(parts[0])
            dv = _try_int(parts[1])
            su = node_of_vertex.get(sv)
            du = node_of_vertex.get(dv)
            if su is None or du is None:
                continue
            b.add_relationship(su, du, "e")
            if params.weighted and len(parts) >= 3:
                w = _try_float(parts[2], 1.0)
                b.set_relationship_prop(su, du, "e", "weight", w)

    return Dataset(b.finalize(), params, vertex_of_node, node_of_vertex)

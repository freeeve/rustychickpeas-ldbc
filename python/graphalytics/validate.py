"""Reference-output validation for the six Graphalytics algorithms (mirrors
src/graphalytics/validate.rs). Outputs are node-indexed; reference files are
`<vertex-id> <value>` per line, so each check maps node -> vertex via the dataset.
Modes: exact (BFS/CDLP), relabel-invariant (WCC), tolerance (PageRank/LCC/SSSP).
Each returns None on success or an error string on the first disagreement.
"""


def parse_reference(text):
    """`<vertex-id> <value>` lines -> {vertex_id: raw_value_string}."""
    m = {}
    for line in text.splitlines():
        it = line.split()
        if len(it) >= 2:
            try:
                m[int(it[0])] = it[1]
            except ValueError:
                pass
    return m


def check_exact_i64(ds, out, reference):
    """Exact integer agreement (BFS depths, CDLP labels)."""
    vof = ds.vertex_of_node
    for node, mine in enumerate(out):
        vertex = vof[node]
        raw = reference.get(vertex)
        if raw is None:
            return f"vertex {vertex} missing from reference"
        if mine != int(raw):
            return f"vertex {vertex}: got {mine}, want {raw}"
    return None


def check_epsilon(ds, out, reference, eps=1e-6):
    """Tolerance float agreement (PageRank, LCC, SSSP); both-infinite (same sign)
    passes, mixed finite/infinite fails."""
    vof = ds.vertex_of_node
    for node, mine in enumerate(out):
        vertex = vof[node]
        raw = reference.get(vertex)
        if raw is None:
            return f"vertex {vertex} missing from reference"
        want = float(raw)
        import math
        if math.isinf(mine) or math.isinf(want):
            ok = math.isinf(mine) and math.isinf(want) and (mine > 0) == (want > 0)
        else:
            ok = abs(mine - want) <= eps
        if not ok:
            return f"vertex {vertex}: got {mine}, want {want} (eps {eps})"
    return None


def check_relabel(ds, out, reference):
    """Relabel-invariant agreement (WCC): the partition `out` induces must equal the
    reference's, regardless of the actual label values (consistent bijection)."""
    vof = ds.vertex_of_node
    ours_to_ref = {}
    ref_to_ours = {}
    for node, mine in enumerate(out):
        vertex = vof[node]
        want = reference.get(vertex)
        if want is None:
            return f"vertex {vertex} missing from reference"
        if mine in ours_to_ref:
            if ours_to_ref[mine] != want:
                return f"vertex {vertex}: our label {mine} maps to both {ours_to_ref[mine]} and {want}"
        else:
            other = ref_to_ours.get(want)
            if other is not None and other != mine:
                return f"vertex {vertex}: ref label {want} maps to both {other} and {mine}"
            ours_to_ref[mine] = want
            ref_to_ours[want] = mine
    return None

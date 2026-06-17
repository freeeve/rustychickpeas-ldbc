#!/usr/bin/env python3
"""Graphalytics WCC + PageRank head-to-head against Kùzu.

Kùzu's `algo` extension covers only WCC and PageRank of the six LDBC Graphalytics
algorithms (no CDLP/LCC, and BFS/SSSP would be awkward Cypher), so this compares
just those two. Correctness is already ground-truthed against the official
reference outputs by the `graphalytics` bin, so this is a timing comparison plus a
WCC partition cross-check. PageRank values are NOT compared: Kùzu runs to its own
convergence/normalization, which differs from LDBC's fixed-iteration formulation.

Load = COPY into an in-memory database (the analogue of the rust loader building
its CSR); project_graph + each algorithm CALL are timed separately (median of N).

Usage: run_graphalytics.py [dataset-dir] [name]   (default: data/graphalytics wiki-Talk)
"""
import os
import statistics
import sys
import tempfile
import time

import kuzu

DATADIR = sys.argv[1] if len(sys.argv) > 1 else "data/graphalytics"
NAME = sys.argv[2] if len(sys.argv) > 2 else "wiki-Talk"


def relabel_ok(ours, ref):
    """Relabel-invariant WCC check: the partition `ours` induces must equal the
    reference's, regardless of label values. Enforces a consistent bijection."""
    fwd, rev = {}, {}
    for v, lab in ours.items():
        rv = ref.get(v)
        if rv is None:
            return f"FAIL: vertex {v} missing from reference"
        if lab in fwd and fwd[lab] != rv:
            return f"FAIL: our label {lab} maps to {fwd[lab]} and {rv}"
        if rv in rev and rev[rv] != lab:
            return f"FAIL: ref label {rv} maps to {rev[rv]} and {lab}"
        fwd[lab], rev[rv] = rv, lab
    return "PASS"


def timed(conn, cypher, runs=3):
    """Median wall-clock ms over `runs` executions, result fully consumed. Pair with
    a `RETURN count(*)` so the algorithm runs to completion entirely inside Kùzu
    (one aggregate row back), with no Python per-row iteration tax on the timing."""
    samples = []
    for _ in range(runs):
        t = time.perf_counter()
        r = conn.execute(cypher)
        while r.has_next():
            r.get_next()
        samples.append((time.perf_counter() - t) * 1000.0)
    return statistics.median(samples)


def main():
    vpath = os.path.join(DATADIR, f"{NAME}.v")
    epath = os.path.join(DATADIR, f"{NAME}.e")

    # Kùzu infers the file format from the extension, so expose the .v/.e files
    # under .csv symlinks for its CSV reader.
    tmp = tempfile.mkdtemp()
    vcsv, ecsv = os.path.join(tmp, "nodes.csv"), os.path.join(tmp, "edges.csv")
    os.symlink(os.path.abspath(vpath), vcsv)
    os.symlink(os.path.abspath(epath), ecsv)

    db = kuzu.Database()  # in-memory
    conn = kuzu.Connection(db)
    conn.execute("CREATE NODE TABLE N(id INT64, PRIMARY KEY(id))")
    conn.execute("CREATE REL TABLE E(FROM N TO N)")

    t = time.perf_counter()
    conn.execute(f"COPY N FROM '{vcsv}' (HEADER=false)")
    conn.execute(f"COPY E FROM '{ecsv}' (HEADER=false, DELIM=' ')")
    load_s = time.perf_counter() - t
    nrows = conn.execute("MATCH (n:N) RETURN count(n)").get_next()[0]
    erows = conn.execute("MATCH ()-[e:E]->() RETURN count(e)").get_next()[0]
    print(f"Kùzu loaded {NAME}: {nrows} nodes, {erows} edges  [COPY {load_s:.2f}s]")

    t = time.perf_counter()
    conn.execute("CALL project_graph('G', ['N'], ['E'])")
    proj_ms = (time.perf_counter() - t) * 1000.0
    print(f"  project_graph: {proj_ms:.1f} ms")

    # Match LDBC's PageRank parameters (10 iterations, damping 0.85). The 1-iteration
    # run is a diagnostic: PR(10)-PR(1) is ~9 iterations of real work, while PR(1) is
    # roughly the per-CALL fixed cost (incl. any graph materialization).
    pr_ms = timed(conn, "CALL page_rank('G', maxIterations := 10, dampingFactor := 0.85) RETURN count(*)")
    pr1_ms = timed(conn, "CALL page_rank('G', maxIterations := 1, dampingFactor := 0.85) RETURN count(*)", runs=2)
    wcc_ms = timed(conn, "CALL weakly_connected_components('G') RETURN count(*)")

    print("\nKùzu algorithm timings (median, full compute via count(*)):")
    print(f"  PR  (page_rank, 10 iters):         {pr_ms:8.1f} ms")
    print(f"  PR  (page_rank,  1 iter, diag):    {pr1_ms:8.1f} ms   (~per-CALL fixed cost)")
    print(f"  WCC (weakly_connected_components): {wcc_ms:8.1f} ms")

    # WCC partition cross-check: a separate, untimed run that collects the labels.
    refpath = os.path.join(DATADIR, f"{NAME}-WCC")
    if os.path.exists(refpath):
        ref = {}
        with open(refpath) as f:
            for line in f:
                p = line.split()
                if len(p) >= 2:
                    ref[int(p[0])] = p[1]
        r = conn.execute("CALL weakly_connected_components('G') RETURN node.id AS id, group_id")
        ours = {}
        while r.has_next():
            row = r.get_next()
            ours[int(row[0])] = str(row[1])
        print(f"  WCC vs official reference (relabel): {relabel_ok(ours, ref)}")


if __name__ == "__main__":
    main()

"""Run the six LDBC Graphalytics algorithms in Python over a `.v`/`.e`/`.properties`
dataset, validating each against any present `<name>-<ALGO>` reference and printing
per-algorithm wall-clock time. Mirrors src/bin/graphalytics.rs.

Usage: python run_graphalytics.py [dataset-dir] [dataset-name]
  (defaults: data/graphalytics / example-directed)
"""

import os
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from graphalytics import algos, load, validate  # noqa: E402

REPO_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DEFAULT_DIR = os.path.join(REPO_ROOT, "data/graphalytics")


def _reference(directory, name, algo):
    path = os.path.join(directory, f"{name}-{algo}")
    if not os.path.exists(path):
        return None
    with open(path, encoding="utf-8") as fh:
        return validate.parse_reference(fh.read())


def main():
    directory = sys.argv[1] if len(sys.argv) > 1 else DEFAULT_DIR
    name = sys.argv[2] if len(sys.argv) > 2 else "example-directed"

    t = time.perf_counter()
    ds = load.load(directory, name)
    load_s = time.perf_counter() - t
    g, p, n, d = ds.graph, ds.params, len(ds), ds.params.directed
    print(f"Loaded {name}: {n} nodes, directed={d}  [{load_s:.2f}s]")

    # Which algorithms this dataset declares a reference for (SSSP often absent on
    # unweighted graphs like wiki-Talk).
    specs = []
    bfs_src = (ds.node(p.bfs_source) if p.bfs_source is not None else None) or 0
    sssp_src = (ds.node(p.sssp_source) if p.sssp_source is not None else None) or 0
    specs.append(("BFS", lambda: algos.bfs(g, bfs_src, n, d), "exact"))
    specs.append(("PR", lambda: algos.pagerank(g, n, d, p.pr_damping, p.pr_iterations), "eps"))
    specs.append(("WCC", lambda: algos.wcc(g, n), "relabel"))
    specs.append(("CDLP", lambda: algos.cdlp(g, n, d, p.cdlp_iterations, ds.vertex_of_node), "exact"))
    specs.append(("LCC", lambda: algos.lcc(g, n, d), "eps"))
    specs.append(("SSSP", lambda: algos.sssp(g, sssp_src, n, d, p.weighted), "eps"))

    print(f"\n{'algo':<6}{'ms':>11}{'n':>10}  validation")
    print("-" * 44)
    for algo, fn, mode in specs:
        ref = _reference(directory, name, algo)
        if ref is None:
            continue  # dataset doesn't supply this algorithm's reference
        t = time.perf_counter()
        out = fn()
        ms = (time.perf_counter() - t) * 1000.0
        if mode == "exact":
            err = validate.check_exact_i64(ds, out, ref)
        elif mode == "relabel":
            err = validate.check_relabel(ds, out, ref)
        else:
            err = validate.check_epsilon(ds, out, ref)
        verdict = "PASS" if err is None else f"FAIL: {err}"
        print(f"{algo:<6}{ms:>11.2f}{len(out):>10}  {verdict}")


if __name__ == "__main__":
    main()

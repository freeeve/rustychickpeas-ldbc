#!/usr/bin/env python3
"""Time the FAIR (WCC-based) LDBC BI Q12 on Kùzu.

The reply-forest is computed once via ``PROJECT_GRAPH`` + ``WEAKLY_CONNECTED_COMPONENTS``
— the equivalent of the rustychickpeas client's native ``roots_via`` — instead of the
pathological ``replyOf*0..30`` recursion (which explodes to ~63 s upward / ~39 s
downward). Reports:

  (a) apples-to-apples = projection + WCC label pass + steady sub-queries + reduction
  (b) steady-state     = projection / component labels cached

and verifies the result is 86 buckets (matches the client + the naive recursive form).

``q12_fair(database, runs)`` is importable (side-effect-free) so other timing
harnesses (e.g. ``time_bi_fair.py``) can reuse this exact pipeline.

Usage:  .venv-kuzu/bin/python kuzu/time_q12_fair.py     # needs kuzu/db-sf1-faithful
"""
import os
import statistics
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
if len(sys.argv) < 3:
    sys.argv = [sys.argv[0], "/tmp", "sf1"]  # run_faithful reads argv at import time
import kuzu  # noqa: E402
import run_faithful as rf  # noqa: E402

DEFAULT_DB = os.path.join(HERE, "db-sf1-faithful")

PROJECT = "CALL PROJECT_GRAPH('rg', ['Message'], ['replyOf'])"
# Each thread's root is the single Post a Forum is the containerOf; the qualifying
# messages are short, content-bearing, recent. Neither sub-query recomputes WCC —
# the component label is joined in from the cached map.
SUB_ROOT = (
    "MATCH (forum:Forum)-[:containerOf]->(post:Message) "
    "WHERE post.lang IN ['ar','hu'] RETURN post.id AS mid"
)
SUB_QUAL = (
    "MATCH (m:Message)<-[:hasCreator]-(p:Person) "
    "WHERE m.length < 20 AND m.hasContent = true AND m.cdate > date('2010-07-22') "
    "RETURN m.id AS mid, p.id AS pid"
)


def _project(conn):
    """Materialise the replyOf reply-forest (idempotent per connection)."""
    try:
        conn.execute(PROJECT)
    except Exception:
        pass  # already projected on this connection


def _wcc_compmap(conn):
    """One WCC pass labelling every message with its thread component."""
    return conn.execute(
        "CALL WEAKLY_CONNECTED_COMPONENTS('rg') RETURN node.id AS mid, group_id AS comp"
    ).get_as_df().astype("int64")


def histogram(permsg, total):
    """Persons-per-message-count histogram, with the zero bucket filled in."""
    hist = {}
    for c in permsg.values():
        hist[c] = hist.get(c, 0) + 1
    hist[0] = hist.get(0, 0) + (total - len(permsg))
    return sorted(([mc, pc] for mc, pc in hist.items()), key=lambda r: (-r[1], -r[0]))


def steady_split(conn, compmap, total):
    """Steady-state work given the cached component map. Returns (kuzu_ms, python_ms, rows)."""
    tk = time.time()
    a = conn.execute(SUB_ROOT).get_as_df()
    b = conn.execute(SUB_QUAL).get_as_df()
    kuzu_ms = (time.time() - tk) * 1000
    tp = time.time()
    arhu = set(a.merge(compmap, on="mid")["comp"])
    b = b.merge(compmap, on="mid")
    permsg = b[b["comp"].isin(arhu)].groupby(b["pid"].astype("int64")).size().to_dict()
    rows = histogram(permsg, total)
    return kuzu_ms, (time.time() - tp) * 1000, rows


def q12_fair(database, runs=5):
    """Time the fair WCC Q12 pipeline. Returns a dict of medians and the result rows.

    (a) apples-to-apples = projection + WCC label pass + steady sub-queries + reduction.
    (b) steady-state     = projection / component labels assumed cached.
    """
    conn = kuzu.Connection(database)
    total = int(conn.execute(rf.q12_person_count()).get_as_df()["cnt"].iloc[0])

    proj = []
    for _ in range(runs):  # fresh connection each run so the projection is re-timed
        c = kuzu.Connection(database)
        t = time.time()
        c.execute(PROJECT)
        proj.append((time.time() - t) * 1000)

    _project(conn)
    wcc, compmap = [], None
    for _ in range(runs):
        t = time.time()
        compmap = _wcc_compmap(conn)
        wcc.append((time.time() - t) * 1000)

    ks, ps, rows = [], [], None
    for _ in range(runs):
        k, p, rows = steady_split(conn, compmap, total)
        ks.append(k)
        ps.append(p)

    m_proj, m_wcc = statistics.median(proj), statistics.median(wcc)
    m_steady = statistics.median([k + p for k, p in zip(ks, ps)])
    return {
        "proj_ms": m_proj, "proj_min": min(proj),
        "wcc_ms": m_wcc, "wcc_min": min(wcc), "n_nodes": len(compmap),
        "steady_ms": m_steady,
        "steady_kuzu_ms": statistics.median(ks), "steady_py_ms": statistics.median(ps),
        "a_ms": m_proj + m_wcc + m_steady, "b_ms": m_steady,
        "buckets": len(rows), "rows": rows,
    }


def main():
    runs = 7
    r = q12_fair(kuzu.Database(DEFAULT_DB), runs=runs)
    print(f"=== Kùzu {kuzu.__version__}  fair Q12  (db-sf1-faithful, median of {runs}) ===")
    print(f"  PROJECT_GRAPH('rg')                {r['proj_ms']:8.1f} ms  (min {r['proj_min']:.1f})")
    print(f"  WCC -> component map ({r['n_nodes']} nodes) {r['wcc_ms']:8.1f} ms  (min {r['wcc_min']:.1f})")
    print(f"  steady sub-queries + reduction     {r['steady_ms']:8.1f} ms")
    print(f"  (a) apples-to-apples = proj+WCC+steady = {r['a_ms']:.1f} ms")
    print(f"  (b) steady-state (labels cached)        = {r['b_ms']:.1f} ms")
    print(f"  PARITY: {r['buckets']} buckets (expect 86) -> {'OK' if r['buckets'] == 86 else 'MISMATCH'}")


if __name__ == "__main__":
    main()

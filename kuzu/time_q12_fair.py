#!/usr/bin/env python3
"""Time the FAIR (WCC-based) LDBC BI Q12 on Kùzu.

The reply-forest is computed once via ``PROJECT_GRAPH`` + ``WEAKLY_CONNECTED_COMPONENTS``
— the equivalent of the rustychickpeas client's native ``roots_via`` — instead of the
pathological ``replyOf*0..30`` recursion (which explodes to ~63 s upward / ~39 s
downward). Reports two operating points:

  (a) apples-to-apples : projection + WCC label pass + reduction, forest rebuilt each
                         call. The WCC labels are pulled once and the per-person
                         reduction over the 1.16 M qualifying messages is done in numpy
                         (membership via searchsorted into the small ar/hu-thread mid
                         set + np.unique/np.bincount) — ~1.45x faster than the pandas
                         hash-join it replaced. This is WCC + scan bound (~1.0 s);
                         pushing the reduction into Kùzu does not help the cold path
                         (the label round-trip costs as much as it saves).
  (b) steady-state     : projection / WCC labels cached. The labels live in a Kùzu side
                         table (``CompLabel``) and the reduction runs *inside* Kùzu — a
                         server-side join + GROUP BY that ships only the ~3.9 k per-person
                         counts to Python, never the 1.16 M qualifying / 2.86 M label
                         sets. This is the win from pushing the reduction into Kùzu.

Both verify 86 buckets (matches the client + the naive recursive form). ``CompLabel`` is
created from the already-materialised label map and dropped again, so the database is
left pristine. ``q12_fair(database, runs)`` is importable (side-effect-free) so other
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
import numpy as np  # noqa: E402
import kuzu  # noqa: E402
import run_faithful as rf  # noqa: E402

DEFAULT_DB = os.path.join(HERE, "db-sf1-faithful")

PROJECT = "CALL PROJECT_GRAPH('rg', ['Message'], ['replyOf'])"
# Each thread's root is the single Post a Forum is the containerOf; the qualifying
# messages are short, content-bearing, recent.
SUB_ROOT = (
    "MATCH (forum:Forum)-[:containerOf]->(post:Message) "
    "WHERE post.lang IN ['ar','hu'] RETURN post.id AS mid"
)
SUB_QUAL = (
    "MATCH (m:Message)<-[:hasCreator]-(p:Person) "
    "WHERE m.length < 20 AND m.hasContent = true AND m.cdate > date('2010-07-22') "
    "RETURN m.id AS mid, p.id AS pid"
)
# Steady-state, server-side: the component label is joined in from the cached CompLabel
# side table, so Kùzu does the ar/hu filter, the qualifying filter and the per-person
# GROUP BY — only the ~3.9 k per-person counts cross to Python.
JOIN_CL = """
MATCH (forum:Forum)-[:containerOf]->(root:Message) WHERE root.lang IN ['ar','hu']
MATCH (rl:CompLabel {mid: root.id})
WITH DISTINCT rl.comp AS arhu
MATCH (m:Message)<-[:hasCreator]-(p:Person)
WHERE m.length < 20 AND m.hasContent = true AND m.cdate > date('2010-07-22')
MATCH (ml:CompLabel {mid: m.id})
WHERE ml.comp = arhu
RETURN p.id AS pid, count(*) AS cnt
"""


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


def _membership(haystack_sorted, probe):
    """Vectorised ``probe in haystack`` via searchsorted into a small sorted array
    (far more cache-friendly than probing the full 2.86 M label map)."""
    idx = np.minimum(np.searchsorted(haystack_sorted, probe), len(haystack_sorted) - 1)
    return haystack_sorted[idx] == probe


def steady_cold(conn, mid_all, comp_all, comp_max, total):
    """Cold (a) reduction: fetch the (unchanged) sub-queries, reduce in numpy.

    The ar/hu component set comes from the root posts; qualifying messages whose
    component is in that set are masked via membership against the small (~64 k)
    ar/hu-thread mid set; per-person counts via ``np.unique`` and the histogram via
    ``np.bincount`` — no 2.86 M-row pandas hash-join rebuilt on every call.
    Returns (kuzu_ms, python_ms, rows)."""
    tk = time.time()
    root_mid = conn.execute(SUB_ROOT).get_as_df()["mid"].to_numpy()
    bq = conn.execute(SUB_QUAL).get_as_df()
    qual_mid, qual_pid = bq["mid"].to_numpy(), bq["pid"].to_numpy()
    kuzu_ms = (time.time() - tk) * 1000

    tp = time.time()
    is_root = _membership(np.sort(root_mid), mid_all)
    arhu_mask = np.zeros(comp_max + 1, dtype=bool)
    arhu_mask[comp_all[is_root]] = True
    thread_mids = np.sort(mid_all[arhu_mask[comp_all]])      # mids in ar/hu threads
    keep = _membership(thread_mids, qual_mid)
    _, counts = np.unique(qual_pid[keep], return_counts=True)
    hb = np.bincount(counts) if len(counts) else np.zeros(1, dtype=np.int64)
    hb[0] += total - len(counts)                            # the zero bucket
    rows = [[int(c), int(hb[c])] for c in range(len(hb)) if hb[c] > 0]
    rows.sort(key=lambda r: (-r[1], -r[0]))
    return kuzu_ms, (time.time() - tp) * 1000, rows


def _build_complabel(conn, compmap):  # noqa: ARG001  (compmap scanned by variable name)
    """Load the cached label map into a Kùzu side table for server-side joins."""
    try:
        conn.execute("DROP TABLE CompLabel")
    except Exception:
        pass
    conn.execute("CREATE NODE TABLE CompLabel(mid INT64, comp INT64, PRIMARY KEY(mid))")
    conn.execute("COPY CompLabel FROM compmap")


def _drop_complabel(conn):
    try:
        conn.execute("DROP TABLE CompLabel")
    except Exception:
        pass


def steady_server(conn, total):
    """Steady (b) reduction executed inside Kùzu; only per-person counts return."""
    df = conn.execute(JOIN_CL).get_as_df()
    permsg = {int(p): int(c) for p, c in zip(df["pid"], df["cnt"])}
    return histogram(permsg, total), len(df)


def q12_fair(database, runs=5):
    """Time the fair WCC Q12 pipeline. Returns a dict of medians and the result rows."""
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
    mid_all = compmap["mid"].to_numpy()
    comp_all = compmap["comp"].to_numpy()
    comp_max = int(comp_all.max())

    ks, ps, rows = [], [], None
    for _ in range(runs):
        k, p, rows = steady_cold(conn, mid_all, comp_all, comp_max, total)
        ks.append(k)
        ps.append(p)

    _build_complabel(conn, compmap)
    try:
        srv, rows_b, n_persons = [], None, 0
        for _ in range(runs):
            t = time.time()
            rows_b, n_persons = steady_server(conn, total)
            srv.append((time.time() - t) * 1000)
    finally:
        _drop_complabel(conn)

    assert len(rows) == len(rows_b) == 86, f"parity broke: {len(rows)} / {len(rows_b)}"

    m_proj, m_wcc = statistics.median(proj), statistics.median(wcc)
    m_cold = statistics.median([k + p for k, p in zip(ks, ps)])
    m_server = statistics.median(srv)
    return {
        "proj_ms": m_proj, "proj_min": min(proj),
        "wcc_ms": m_wcc, "wcc_min": min(wcc), "n_nodes": len(compmap),
        "steady_cold_ms": m_cold,
        "steady_kuzu_ms": statistics.median(ks), "steady_py_ms": statistics.median(ps),
        "steady_server_ms": m_server, "server_min": min(srv), "n_persons": n_persons,
        "a_ms": m_proj + m_wcc + m_cold,     # cold, forest rebuilt, numpy reduce
        "b_ms": m_server,                    # warm, labels cached, server-side reduce
        "buckets": len(rows), "rows": rows,
    }


def main():
    runs = 7
    r = q12_fair(kuzu.Database(DEFAULT_DB), runs=runs)
    print(f"=== Kùzu {kuzu.__version__}  fair Q12  (db-sf1-faithful, median of {runs}) ===")
    print(f"  PROJECT_GRAPH('rg')                {r['proj_ms']:8.1f} ms  (min {r['proj_min']:.1f})")
    print(f"  WCC -> component map ({r['n_nodes']} nodes) {r['wcc_ms']:8.1f} ms  (min {r['wcc_min']:.1f})")
    print(f"  cold reduce (numpy)                {r['steady_cold_ms']:8.1f} ms"
          f"  [Kùzu fetch {r['steady_kuzu_ms']:.1f} + numpy {r['steady_py_ms']:.1f}]")
    print(f"  steady reduce (server-side join)   {r['steady_server_ms']:8.1f} ms"
          f"  (min {r['server_min']:.1f}, {r['n_persons']} persons to Python)")
    print(f"  (a) apples-to-apples (forest rebuilt)    = {r['a_ms']:.1f} ms")
    print(f"  (b) steady-state (labels cached in Kùzu) = {r['b_ms']:.1f} ms")
    print(f"  PARITY: {r['buckets']} buckets (expect 86) -> {'OK' if r['buckets'] == 86 else 'MISMATCH'}")


if __name__ == "__main__":
    main()

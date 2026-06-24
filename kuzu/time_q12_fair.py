#!/usr/bin/env python3
"""Time the FAIR (WCC-based) LDBC BI Q12 on Kùzu.

The reply-forest is computed once via ``PROJECT_GRAPH`` + ``WEAKLY_CONNECTED_COMPONENTS``
— the equivalent of the rustychickpeas client's native ``roots_via`` — instead of the
pathological ``replyOf*0..30`` recursion (which explodes to ~63 s upward / ~39 s
downward). Reports:

  (a) apples-to-apples = projection + WCC label pass + steady sub-queries + reduction
  (b) steady-state     = projection / component labels cached

and verifies the result is 86 buckets (matches the client + the naive recursive form).

Usage:  .venv-kuzu/bin/python kuzu/time_q12_fair.py     # needs kuzu/db-sf1-faithful
"""
import os
import statistics
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
sys.argv = [sys.argv[0], "/tmp", "sf1"]  # run_faithful reads argv at import time
import kuzu  # noqa: E402
import run_faithful as rf  # noqa: E402

DB = os.path.join(HERE, "db-sf1-faithful")
RUNS = 7
database = kuzu.Database(DB)
conn = kuzu.Connection(database)
conn.execute("CALL PROJECT_GRAPH('rg', ['Message'], ['replyOf'])")


def stat(fn, runs=RUNS):
    s = []
    out = None
    for _ in range(runs):
        t = time.time()
        out = fn()
        s.append((time.time() - t) * 1000)
    return statistics.median(s), min(s), out


def histogram(permsg, total):
    hist = {}
    for c in permsg.values():
        hist[c] = hist.get(c, 0) + 1
    hist[0] = hist.get(0, 0) + (total - len(permsg))
    return sorted(([mc, pc] for mc, pc in hist.items()), key=lambda r: (-r[1], -r[0]))


TOTAL = int(conn.execute(rf.q12_person_count()).get_as_df()["cnt"].iloc[0])

# ---- one-time reply-forest setup (the roots_via equivalent) ------------------
m_proj, n_proj, _ = stat(
    lambda: kuzu.Connection(database).execute("CALL PROJECT_GRAPH('rg', ['Message'], ['replyOf'])")
)
m_wccm, n_wccm, compmap = stat(
    lambda: conn.execute(
        "CALL WEAKLY_CONNECTED_COMPONENTS('rg') RETURN node.id AS mid, group_id AS comp"
    ).get_as_df()
)
compmap = compmap.astype("int64")

# ---- steady-state sub-queries (NO WCC) + Python reduction --------------------
SUB_ROOT = (
    "MATCH (forum:Forum)-[:containerOf]->(post:Message) "
    "WHERE post.lang IN ['ar','hu'] RETURN post.id AS mid"
)
SUB_QUAL = (
    "MATCH (m:Message)<-[:hasCreator]-(p:Person) "
    "WHERE m.length < 20 AND m.hasContent = true AND m.cdate > date('2010-07-22') "
    "RETURN m.id AS mid, p.id AS pid"
)


def steady_split():
    """Returns (kuzu_ms, python_ms, rows) given the cached component map."""
    tk = time.time()
    a = conn.execute(SUB_ROOT).get_as_df()
    b = conn.execute(SUB_QUAL).get_as_df()
    kuzu_ms = (time.time() - tk) * 1000
    tp = time.time()
    arhu = set(a.merge(compmap, on="mid")["comp"])
    b = b.merge(compmap, on="mid")
    permsg = b[b["comp"].isin(arhu)].groupby(b["pid"].astype("int64")).size().to_dict()
    rows = histogram(permsg, TOTAL)
    return kuzu_ms, (time.time() - tp) * 1000, rows


ks, ps, rows = [], [], None
for _ in range(RUNS):
    k, p, rows = steady_split()
    ks.append(k)
    ps.append(p)
m_steady = statistics.median([k + p for k, p in zip(ks, ps)])

print(f"=== Kùzu {kuzu.__version__}  fair Q12  (db-sf1-faithful, median of {RUNS}) ===")
print(f"  PROJECT_GRAPH('rg')                {m_proj:8.1f} ms  (min {n_proj:.1f})")
print(f"  WCC -> component map ({len(compmap)} nodes) {m_wccm:8.1f} ms  (min {n_wccm:.1f})")
print(f"  steady sub-queries + reduction     {m_steady:8.1f} ms")
print(f"  (a) apples-to-apples = proj+WCC+steady = {m_proj + m_wccm + m_steady:.1f} ms")
print(f"  (b) steady-state (labels cached)        = {m_steady:.1f} ms")
print(f"  PARITY: {len(rows)} buckets (expect 86) -> {'OK' if len(rows) == 86 else 'MISMATCH'}")

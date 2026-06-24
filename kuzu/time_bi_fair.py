#!/usr/bin/env python3
"""Time all six LDBC BI head-to-head queries on Kùzu in ONE process (median of 5),
so the numbers are internally comparable rather than stitched from separate runs.

Fair formulations throughout:
  - Q1 / Q2 / Q5 / Q7 : the faithful Cypher from run_faithful.py.
  - Q6                : tuned DISTINCT-(person1, person2) form — collapses the
                        per-row 2-hop like re-expansion the original did.
  - Q12               : the WCC reply-forest pipeline from time_q12_fair.py,
                        reported as the apples-to-apples (a) = projection + WCC + steady.

Result counts are asserted (Q1=12 groups, Q2/Q5/Q6/Q7=100, Q12=86 buckets).

Usage:  .venv-kuzu/bin/python kuzu/time_bi_fair.py   # needs kuzu/db-sf1-faithful
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
import time_q12_fair as q12f  # noqa: E402

DB = os.path.join(HERE, "db-sf1-faithful")
RUNS = 5

# Fair Q6: collapse to DISTINCT (person1, person2) before the 2-hop like expansion.
# Each liker's received-like set is disjoint, so count(like) per liker summed over a
# person1's distinct likers equals the original count(DISTINCT like) — value-identical,
# without re-expanding the like-set once per (person1, message1) row.
Q6_FAIR = """
MATCH (tag:Tag {name: 'Arnold_Schwarzenegger'})<-[:hasTag]-(message1:Message)<-[:hasCreator]-(person1:Person),
      (message1)<-[:likes]-(person2:Person)
WITH DISTINCT person1, person2
MATCH (person2)-[:hasCreator]->(:Message)<-[like:likes]-(:Person)
WITH person1, count(like) AS auth
RETURN person1.id AS pid, sum(auth) AS authorityScore
ORDER BY authorityScore DESC, pid LIMIT 100
"""


def med(conn, cypher, runs=RUNS):
    """Median wall-ms over `runs` executions (one warm-up for the row count)."""
    n = len(conn.execute(cypher).get_as_df())
    s = []
    for _ in range(runs):
        t = time.time()
        conn.execute(cypher).get_as_df()
        s.append((time.time() - t) * 1000)
    return statistics.median(s), n


def main():
    try:
        load = os.getloadavg()
    except OSError:
        load = (0.0, 0.0, 0.0)
    database = kuzu.Database(DB)
    conn = kuzu.Connection(database)
    print(f"=== Kùzu {kuzu.__version__}  LDBC BI fair head-to-head  (db-sf1-faithful, median of {RUNS}) ===")
    print(f"    loadavg(1/5/15m) = {load[0]:.2f} / {load[1]:.2f} / {load[2]:.2f}\n")

    rows = []
    m, n = med(conn, rf.Q1)
    rows.append(("Q1 posting summary", m, f"{n} groups", n == 12))
    m, n = med(conn, rf.q2_text())
    rows.append(("Q2 tag evolution", m, f"{n} tags", n == 100))
    m, n = med(conn, rf.Q5)
    rows.append(("Q5 active posters", m, f"{n} rows", n == 100))
    m, n = med(conn, Q6_FAIR)
    rows.append(("Q6 authoritative users", m, f"{n} rows", n == 100))
    m, n = med(conn, rf.Q7)
    rows.append(("Q7 related topics", m, f"{n} rows", n == 100))
    q12 = q12f.q12_fair(database, runs=RUNS)  # (a) = projection + WCC + steady
    rows.append(("Q12 message histogram", q12["a_ms"], f"{q12['buckets']} buckets", q12["buckets"] == 86))

    w = max(len(r[0]) for r in rows)
    print(f"  {'query':<{w}}   {'kuzu_ms':>10}   {'result':<11} parity")
    print(f"  {'-' * w}   {'-' * 10}   {'-' * 11} ------")
    all_ok = True
    for name, ms, result, ok in rows:
        all_ok &= ok
        print(f"  {name:<{w}}   {ms:>10.2f}   {result:<11} {'OK' if ok else 'FAIL'}")

    print(f"\n  Q12 (a) cold = projection {q12['proj_ms']:.1f} ms + WCC {q12['wcc_ms']:.1f} ms "
          f"+ pandas reduce {q12['steady_pandas_ms']:.1f} ms "
          f"(Kùzu {q12['steady_kuzu_ms']:.1f} + Python {q12['steady_py_ms']:.1f}) = {q12['a_ms']:.1f} ms")
    print(f"  Q12 (b) steady-state: labels cached in Kùzu, server-side join+GROUP BY "
          f"({q12['n_persons']} persons to Python) = {q12['steady_server_ms']:.1f} ms")
    print(f"\n  PARITY: {'ALL OK' if all_ok else 'FAILURES PRESENT'}")
    return 0 if all_ok else 1


if __name__ == "__main__":
    sys.exit(main())

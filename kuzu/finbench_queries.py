#!/usr/bin/env python3
"""Kùzu reference for the FinBench complex reads (head-to-head with the Rust
impls). Runs faithful-ish Cypher against the db built by finbench_import.py,
times each query (median of N), and prints the result count.

    .venv-kuzu/bin/python kuzu/finbench_queries.py [db_path]

Notes: a parameter inside a recursive-rel list comprehension `all(x IN rels(e)
…)` crashes Kùzu (assertion), so the time-window bounds are inlined as literals
(they are integers we control); seed ids stay as params. Full window so results
are non-empty. Timing-only; truncation-on-hub choke points are not replicated.
"""
import re
import statistics
import sys
import time

import kuzu

DB = sys.argv[1] if len(sys.argv) > 1 else "kuzu/db-finbench-sf10"
conn = kuzu.Connection(kuzu.Database(DB))
S, E = 0, 2**62  # window bounds, inlined as literals


def one(cypher, params):
    r = conn.execute(cypher, params)
    rows = 0
    while r.has_next():
        r.get_next()
        rows += 1
    return rows


def timed(name, cypher, runs=7):
    params = {k: ALL[k] for k in set(re.findall(r"\$(\w+)", cypher)) if k in ALL}
    try:
        rows = one(cypher, params)  # warmup + row count
        ts = []
        for _ in range(runs):
            t = time.perf_counter()
            one(cypher, params)
            ts.append((time.perf_counter() - t) * 1000)
        print(f"  {name:<26} {statistics.median(ts):8.2f} ms   (rows={rows})")
    except Exception as ex:
        print(f"  {name:<26} ERROR: {str(ex)[:120]}")


# --- seeds (match the Rust bin's shape: high-degree account / guarantor) ---
acct = conn.execute(
    "MATCH (a:Account)-[t:transfer]->() RETURN a.id, count(t) AS c ORDER BY c DESC LIMIT 1"
).get_next()[0]
dst = conn.execute(
    "MATCH (a:Account)-[:transfer]->()-[:transfer]->(d:Account) WHERE a.id = $a RETURN d.id LIMIT 1",
    {"a": acct},
).get_next()[0]
person = conn.execute(
    "MATCH (p:Person)-[g:personGuarantee]->() RETURN p.id, count(g) AS c ORDER BY c DESC LIMIT 1"
).get_next()[0]
print(f"DB {DB}  seeds: account={acct} dst={dst} person={person}")

ALL = {"acct": acct, "dst": dst, "person": person, "minamt": 1000.0}

# CR1 — blocked-medium related accounts (<=3-hop in-window reverse transfer).
timed("CR1 blocked-medium", f"""
    MATCH (other:Account)-[e:transfer*1..3]->(s:Account)
    WHERE s.id = $acct AND all(x IN rels(e) WHERE x.createTime >= {S} AND x.createTime <= {E})
    MATCH (m:Medium)-[:signIn]->(other) WHERE m.isBlocked
    RETURN DISTINCT other.id, m.id
""")

# CR2 — fund gathered from loan-applying accounts.
timed("CR2 loan-gather", f"""
    MATCH (p:Person)-[:personOwn]->(owned:Account) WHERE p.id = $person
    MATCH (other:Account)-[e:transfer*1..3]->(owned)
    WHERE all(x IN rels(e) WHERE x.createTime >= {S} AND x.createTime <= {E})
    MATCH (l:Loan)-[d:deposit]->(other) WHERE d.createTime >= {S} AND d.createTime <= {E}
    RETURN other.id, sum(l.loanAmount) AS amt, sum(l.balance) AS bal ORDER BY amt DESC
""")

# CR3 — shortest in-window transfer path.
timed("CR3 shortest-path", f"""
    MATCH (a:Account), (b:Account) WHERE a.id = $acct AND b.id = $dst
    MATCH p = (a)-[e:transfer* SHORTEST 1..10]->(b)
    WHERE all(x IN rels(e) WHERE x.createTime >= {S} AND x.createTime <= {E})
    RETURN length(p) LIMIT 1
""")

# CR4 — three accounts in a time-ordered transfer cycle.
timed("CR4 3-cycle", """
    MATCH (a:Account)-[t1:transfer]->(b:Account)-[t2:transfer]->(c:Account)-[t3:transfer]->(a)
    WHERE a.id = $acct AND b.id <> c.id
      AND t1.createTime < t2.createTime AND t2.createTime < t3.createTime
      AND t1.amount >= $minamt AND t2.amount >= $minamt AND t3.amount >= $minamt
    RETURN a.id, b.id, c.id LIMIT 100
""")

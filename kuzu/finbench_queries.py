#!/usr/bin/env python3
"""Kùzu reference for the FinBench complex reads (head-to-head with the Rust
impls). Runs + times faithful-ish Cypher against the db built by
finbench_import.py.

    .venv-kuzu/bin/python kuzu/finbench_queries.py [db_path]

Kùzu notes worked around here: (1) a parameter inside a recursive-rel list
comprehension `all(x IN rels(e) …)` crashes, so window bounds are inlined as
literals; (2) Kùzu's recursive `*1..3` engine explodes on hub vertices (CR2 went
20 s), so reverse-reachability is unrolled into explicit hops in the driver.
Full window so results are non-empty; timing-only; truncation not replicated.
"""
import re
import statistics
import sys
import time

import kuzu

DB = sys.argv[1] if len(sys.argv) > 1 else "kuzu/db-finbench-sf10"
conn = kuzu.Connection(kuzu.Database(DB))
S, E = 0, 2**62


def run(cypher, params):
    r = conn.execute(cypher, params)
    n = 0
    while r.has_next():
        r.get_next()
        n += 1
    return n


def timed(name, thunk, runs=7):
    """`thunk` returns the row count; timed over `runs` (median)."""
    try:
        rows = thunk()
        ts = []
        for _ in range(runs):
            t = time.perf_counter()
            thunk()
            ts.append((time.perf_counter() - t) * 1000)
        print(f"  {name:<24} {statistics.median(ts):9.2f} ms   (rows={rows})")
    except Exception as ex:
        print(f"  {name:<24} ERROR: {str(ex)[:110]}")


def q(cypher):
    """A thunk for a single Cypher query (params auto-extracted from $refs)."""
    params = {k: SEED[k] for k in set(re.findall(r"\$(\w+)", cypher)) if k in SEED}
    return lambda: run(cypher, params)


def top(cypher):
    return conn.execute(cypher).get_next()[0]


# --- seeds (high-degree / well-connected, per the spec's parameter shape) ---
acct = top("MATCH (a:Account)-[t:transfer]->() RETURN a.id, count(t) AS c ORDER BY c DESC LIMIT 1")
dst = conn.execute(
    "MATCH (a:Account)-[:transfer]->()-[:transfer]->(d:Account) WHERE a.id=$a RETURN d.id LIMIT 1",
    {"a": acct},
).get_next()[0]
person = top("MATCH (p:Person)-[g:personGuarantee]->() RETURN p.id, count(g) AS c ORDER BY c DESC LIMIT 1")
card = top("MATCH (c:Account)-[w:withdraw]->() RETURN c.id, count(w) AS c2 ORDER BY c2 DESC LIMIT 1")
loan = top("MATCH (l:Loan)-[d:deposit]->() RETURN l.id, count(d) AS c ORDER BY c DESC LIMIT 1")
inv = top("MATCH (p:Person)-[i:personInvest]->() RETURN p.id, count(i) AS c ORDER BY c DESC LIMIT 1")
SEED = {"acct": acct, "dst": dst, "person": person, "card": card, "loan": loan, "inv": inv, "minamt": 1000.0}
print(f"DB {DB}\nseeds: account={acct} dst={dst} person={person} card={card} loan={loan} investor={inv}")


# CR2 — reverse-reachable accounts unrolled to explicit hops (recursive = 20s).
def cr2():
    others = set()
    for h in range(1, 4):
        mids = "<-[:transfer]-()" * (h - 1)
        r = conn.execute(
            f"MATCH (p:Person)-[:personOwn]->(o:Account) WHERE p.id=$p "
            f"MATCH (o){mids}<-[:transfer]-(x:Account) RETURN DISTINCT x.id",
            {"p": person},
        )
        while r.has_next():
            others.add(r.get_next()[0])
    if not others:
        return 0
    return run(
        "MATCH (l:Loan)-[d:deposit]->(o:Account) WHERE o.id IN $ids "
        "RETURN o.id, sum(l.loanAmount), sum(l.balance)",
        {"ids": list(others)},
    )


timed("CR1 blocked-medium", q(f"""
    MATCH (s:Account) WHERE s.id=$acct MATCH (s)<-[e:transfer*1..3]-(other:Account)
    WHERE all(x IN rels(e) WHERE x.createTime >= {S} AND x.createTime <= {E})
    MATCH (m:Medium)-[:signIn]->(other) WHERE m.isBlocked
    RETURN DISTINCT other.id, m.id"""))
timed("CR2 loan-gather", cr2)
timed("CR3 shortest-path", q(f"""
    MATCH (a:Account),(b:Account) WHERE a.id=$acct AND b.id=$dst
    MATCH p=(a)-[e:transfer* SHORTEST 1..10]->(b)
    WHERE all(x IN rels(e) WHERE x.createTime >= {S} AND x.createTime <= {E})
    RETURN length(p) LIMIT 1"""))
timed("CR4 3-cycle", q("""
    MATCH (a:Account)-[t1:transfer]->(b:Account)-[t2:transfer]->(c:Account)-[t3:transfer]->(a)
    WHERE a.id=$acct AND b.id<>c.id AND t1.createTime<t2.createTime AND t2.createTime<t3.createTime
      AND t1.amount>=$minamt AND t2.amount>=$minamt AND t3.amount>=$minamt
    RETURN a.id,b.id,c.id LIMIT 100"""))
timed("CR5 downstream-trace", q("""
    MATCH (p:Person)-[:personOwn]->(o:Account)-[t:transfer]->(d:Account)
    WHERE p.id=$person RETURN count(DISTINCT d)"""))
timed("CR6 withdraw-after-in", q("""
    MATCH (src:Account)-[t:transfer]->(card:Account)-[w:withdraw]->(d:Account)
    WHERE card.id=$card AND t.createTime < w.createTime RETURN count(*)"""))
timed("CR7 in-out-ratio", q("""
    MATCH (a:Account) WHERE a.id=$acct MATCH (a)-[out:transfer]->()
    WITH a, sum(out.amount) AS oa, count(out) AS oc
    MATCH (a)<-[inc:transfer]-() RETURN oa, sum(inc.amount) AS ia, oc, count(inc) AS ic"""))
timed("CR8 loan-fund-trace", q("""
    MATCH (l:Loan)-[:deposit]->(a:Account)-[t:transfer]->(b:Account)
    WHERE l.id=$loan RETURN count(DISTINCT b)"""))
timed("CR9 laundering-loan", q("""
    MATCH (l:Loan)-[:deposit]->(a:Account)-[t:transfer]->(b:Account)-[:repay]->(l2:Loan)
    WHERE l.id=$loan AND t.amount>=$minamt RETURN count(*)"""))
timed("CR10 investor-sim", q("""
    MATCH (p1:Person)-[:personInvest]->(c:Company)<-[:personInvest]-(p2:Person)
    WHERE p1.id=$inv AND p1.id<>p2.id RETURN p2.id, count(c) AS common ORDER BY common DESC LIMIT 20"""))
timed("CR11 guarantee-chain", q("""
    MATCH (p:Person)-[:personGuarantee*1..3]->(g:Person)-[:personApply]->(l:Loan)
    WHERE p.id=$person RETURN sum(l.loanAmount) AS exposure"""))
timed("CR12 company-transfer", q("""
    MATCH (p:Person)-[:personOwn]->(a:Account)-[t:transfer]->(ca:Account)<-[:companyOwn]-(c:Company)
    WHERE p.id=$person RETURN c.id, sum(t.amount) AS amt ORDER BY amt DESC LIMIT 20"""))

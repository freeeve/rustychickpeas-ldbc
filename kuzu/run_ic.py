#!/usr/bin/env python3
"""Kùzu reference side for the SNB Interactive (IC) workload — the IC analogue
of run_faithful.py. Connects (read-only) to the existing db-{scale}-faithful
built by run_faithful.py and runs the feasible IC tier with the SAME seeds the
rust pick_seeds chose, so both engines run identical parameters.

Usage:
  run_ic.py [scale] [--emit-json <dir>]

  --emit-json <dir>  emit icN/isN.kuzu.json (matching the rust side's
                     icN/isN.rust.json) for kuzu/compare.py, reading the seeds
                     the rust binary wrote to <dir>/seeds.json. Without it,
                     time each query (median of 5) and print a table.

Covered: IC2, IC9, IC13, IS2, IS3, IS5 (the tier feasible on the faithful
Message/Person/knows/hasCreator/replyOf projection). IC1/IS1 (need Person
firstName/lastName, which the faithful projection omits) and IC14 (interaction
weight semantics) are deferred on the Kùzu side.
"""
import datetime
import json
import os
import sys
import time

import kuzu

# Deterministic pick_seeds defaults for SF1 (person/person_b are LDBC ids).
SEEDS = {"person": 4398046519825, "person_b": 15393162798503, "max_day": 15706}


def main():
    args = sys.argv[1:]
    emit = None
    if "--emit-json" in args:
        i = args.index("--emit-json")
        emit = args[i + 1]
        del args[i : i + 2]
    scale = args[0] if args else "sf1"
    db = f"kuzu/db-{scale}-faithful"

    seeds = dict(SEEDS)
    if emit and os.path.exists(f"{emit}/seeds.json"):
        s = json.load(open(f"{emit}/seeds.json"))
        seeds.update({k: s[k] for k in ("person", "person_b", "max_day") if k in s})
    person, person_b = seeds["person"], seeds["person_b"]
    maxdate = (datetime.date(1970, 1, 1) + datetime.timedelta(days=seeds["max_day"])).isoformat()

    conn = kuzu.Connection(kuzu.Database(db, read_only=True))

    # Each query returns rows as lists; the emit projection (below) reduces each
    # to the engine-independent columns the rust side also emits (ms timestamps
    # and LDBC ids, never internal node ids).
    queries = {
        # IC2: 20 most recent messages by the seed's friends, on/before maxDate.
        "ic2": f"""MATCH (:Person {{id:{person}}})-[:knows]-(f:Person)-[:hasCreator]->(m:Message)
                   WHERE m.cdate <= date('{maxdate}')
                   RETURN m.mts AS mts ORDER BY mts DESC LIMIT 20""",
        # IC9: 20 most recent messages by friends and friends-of-friends (<=2 hops).
        "ic9": f"""MATCH (:Person {{id:{person}}})-[:knows*1..2]-(f:Person)-[:hasCreator]->(m:Message)
                   WHERE f.id <> {person} AND m.cdate <= date('{maxdate}')
                   RETURN DISTINCT m.id AS mid, m.mts AS mts ORDER BY mts DESC, mid LIMIT 20""",
        # IC13: unweighted shortest-path length in the knows graph.
        "ic13": f"""MATCH p = (:Person {{id:{person}}})-[:knows* SHORTEST 1..15]-(:Person {{id:{person_b}}})
                    RETURN length(p) AS hops""",
        # IS2: the seed's own 10 most recent messages.
        "is2": f"""MATCH (:Person {{id:{person}}})-[:hasCreator]->(m:Message)
                   WHERE m.cdate <= date('{maxdate}')
                   RETURN m.mts AS mts ORDER BY mts DESC LIMIT 10""",
        # IS3: the seed's direct friends.
        "is3": f"""MATCH (:Person {{id:{person}}})-[:knows]-(f:Person) RETURN f.id AS fid ORDER BY fid""",
        # IS5: creator of the seed's most recent message (= the seed).
        "is5": f"""MATCH (:Person {{id:{person}}})-[:hasCreator]->(m:Message) WHERE m.cdate <= date('{maxdate}')
                   WITH m ORDER BY m.mts DESC LIMIT 1
                   MATCH (c:Person)-[:hasCreator]->(m) RETURN c.id AS cid""",
    }

    # Map each query's rows to the comparable JSON the rust side emits.
    def project(name, rows):
        if name in ("ic2", "is2"):
            return [[r[0]] for r in rows]          # [mts]
        if name == "ic9":
            return [[r[1]] for r in rows]          # [mts] (col 1)
        return [[r[0]] for r in rows]              # ic13 [hops], is3 [fid], is5 [cid]

    def run(q):
        r = conn.execute(q)
        out = []
        while r.has_next():
            out.append(r.get_next())
        return out

    if emit:
        os.makedirs(emit, exist_ok=True)
        for name, q in queries.items():
            rows = project(name, run(q))
            with open(f"{emit}/{name}.kuzu.json", "w") as f:
                json.dump(rows, f)
            print(f"  emitted {name}.kuzu.json ({len(rows)} rows)")
        return

    print(f"\n=== LDBC SNB Interactive — Kùzu reference ({scale}) ===")
    print(f"Seeds: person={person}, person_b={person_b}, maxDate={maxdate}\n")
    print("Timings (median of 5):")
    runs = 5
    for name, q in queries.items():
        run(q)  # warmup
        samples = []
        for _ in range(runs):
            t = time.time()
            rows = run(q)
            samples.append((time.time() - t) * 1000.0)
        samples.sort()
        print(f"{name:<8} {samples[len(samples)//2]:>9.2f} ms   (rows={len(rows)})")


if __name__ == "__main__":
    main()

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

# Deterministic pick_seeds defaults for SF1 (person/person_b are LDBC ids;
# seed_tag / ic4 window are also read from seeds.json in --emit-json mode).
SEEDS = {
    "person": 4398046519825, "person_b": 15393162798503, "max_day": 15706,
    "seed_tag": "Augustine_of_Hippo", "ic4_start": 14975, "ic4_dur": 365,
}


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
        keys = ("person", "person_b", "max_day", "seed_tag", "ic4_start", "ic4_dur")
        seeds.update({k: s[k] for k in keys if k in s})
    person, person_b, seed_tag = seeds["person"], seeds["person_b"], seeds["seed_tag"]
    epoch = datetime.date(1970, 1, 1)
    maxdate = (epoch + datetime.timedelta(days=seeds["max_day"])).isoformat()
    ic4_start = (epoch + datetime.timedelta(days=seeds["ic4_start"])).isoformat()
    ic4_end = (epoch + datetime.timedelta(days=seeds["ic4_start"] + seeds["ic4_dur"])).isoformat()

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
        # IC4: tags on the seed's friends' Posts in the window, never on their
        # Posts before it. (task 052)
        "ic4": f"""MATCH (p:Person {{id:{person}}})-[:knows]-(:Person)-[:hasCreator]->(post:Message)-[:hasTag]->(t:Tag)
                   WHERE post.isComment = false AND post.cdate >= date('{ic4_start}') AND post.cdate < date('{ic4_end}')
                     AND NOT EXISTS {{ MATCH (p)-[:knows]-(:Person)-[:hasCreator]->(pre:Message)-[:hasTag]->(t)
                                       WHERE pre.isComment = false AND pre.cdate < date('{ic4_start}') }}
                   RETURN t.name AS name, count(DISTINCT post) AS cnt ORDER BY cnt DESC, name ASC LIMIT 10""",
        # IC6: tags co-occurring with seed_tag on the neighbourhood's Posts.
        "ic6": f"""MATCH (:Person {{id:{person}}})-[:knows*1..2]-(f:Person)-[:hasCreator]->(post:Message)-[:hasTag]->(:Tag {{name:'{seed_tag}'}})
                   WHERE post.isComment = false AND f.id <> {person}
                   MATCH (post)-[:hasTag]->(other:Tag) WHERE other.name <> '{seed_tag}'
                   RETURN other.name AS name, count(DISTINCT post) AS cnt ORDER BY cnt DESC, name ASC LIMIT 10""",
        # IC8: 20 most recent replies to the seed's messages.
        "ic8": f"""MATCH (:Person {{id:{person}}})-[:hasCreator]->(:Message)<-[:replyOf]-(reply:Message)
                   RETURN reply.mts AS mts ORDER BY mts DESC LIMIT 20""",
        # IS6: forum + moderator of the seed's newest Post.
        "is6": f"""MATCH (:Person {{id:{person}}})-[:hasCreator]->(m:Message) WHERE m.isComment = false
                   WITH m ORDER BY m.mts DESC LIMIT 1
                   MATCH (forum:Forum)-[:containerOf]->(m) MATCH (forum)-[:hasModerator]->(mod:Person)
                   RETURN forum.id AS fid, mod.id AS modid""",
        # IS7: direct replies to the seed's newest Post + a knows flag.
        "is7": f"""MATCH (p:Person {{id:{person}}})-[:hasCreator]->(m:Message) WHERE m.isComment = false
                   WITH p, m ORDER BY m.mts DESC LIMIT 1
                   MATCH (m)<-[:replyOf]-(reply:Message)<-[:hasCreator]-(author:Person)
                   RETURN reply.mts AS mts, author.id AS aid,
                     CASE WHEN author.id <> p.id AND EXISTS {{ MATCH (author)-[:knows]-(p) }} THEN 1 ELSE 0 END AS knows
                   ORDER BY mts DESC, aid ASC""",
    }

    # Map each query's rows to the comparable JSON the rust side emits.
    def project(name, rows):
        if name in ("ic2", "is2", "ic8"):
            return [[r[0]] for r in rows]              # [mts]
        if name == "ic9":
            return [[r[1]] for r in rows]              # [mts] (col 1)
        if name in ("ic4", "ic6"):
            return [[r[0], r[1]] for r in rows]        # [tagName, count]
        if name == "is6":
            return [[r[0], r[1]] for r in rows]        # [forumId, moderatorId]
        if name == "is7":
            return [[r[0], r[1], r[2]] for r in rows]  # [mts, authorId, knows]
        return [[r[0]] for r in rows]                  # ic13 [hops], is3 [fid], is5 [cid]

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

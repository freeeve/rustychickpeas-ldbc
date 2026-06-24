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
    "first_name": "John", "seed_tag": "Augustine_of_Hippo", "ic4_start": 14975, "ic4_dur": 365,
    "seed_country": "Indonesia", "seed_class": "Saint",
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
        keys = ("person", "person_b", "max_day", "first_name", "seed_tag", "ic4_start",
                "ic4_dur", "seed_country", "seed_class")
        seeds.update({k: s[k] for k in keys if k in s})
    person, person_b = seeds["person"], seeds["person_b"]
    seed_tag, first_name = seeds["seed_tag"], seeds["first_name"]
    seed_country, seed_class = seeds["seed_country"], seeds["seed_class"]
    epoch = datetime.date(1970, 1, 1)
    maxdate = (epoch + datetime.timedelta(days=seeds["max_day"])).isoformat()
    ic4_start = (epoch + datetime.timedelta(days=seeds["ic4_start"])).isoformat()
    ic4_end = (epoch + datetime.timedelta(days=seeds["ic4_start"] + seeds["ic4_dur"])).isoformat()
    # Fixed params matching the rust side's loader-backed smoke calls.
    ic3_start, ic3_end = "2010-01-01", (datetime.date(2010, 1, 1) + datetime.timedelta(days=1500)).isoformat()
    ic5_min = "2011-01-01"

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
        "ic9": f"""MATCH (:Person {{id:{person}}})-[:knows*1..2]-(f:Person) WHERE f.id <> {person}
                   WITH DISTINCT f
                   MATCH (f)-[:hasCreator]->(m:Message) WHERE m.cdate <= date('{maxdate}')
                   RETURN m.id AS mid, m.mts AS mts ORDER BY mts DESC, mid LIMIT 20""",
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
        "ic4": f"""MATCH (:Person {{id:{person}}})-[:knows]-(:Person)-[:hasCreator]->(post:Message)-[:hasTag]->(t:Tag)
                   WHERE post.isComment = false AND post.cdate >= date('{ic4_start}') AND post.cdate < date('{ic4_end}')
                   WITH t, count(DISTINCT post) AS cnt
                   WHERE NOT EXISTS {{ MATCH (:Person {{id:{person}}})-[:knows]-(:Person)-[:hasCreator]->(pre:Message)-[:hasTag]->(t)
                                       WHERE pre.isComment = false AND pre.cdate < date('{ic4_start}') }}
                   RETURN t.name AS name, cnt ORDER BY cnt DESC, name ASC LIMIT 10""",
        # IC6: tags co-occurring with seed_tag on the neighbourhood's Posts.
        "ic6": f"""MATCH (:Person {{id:{person}}})-[:knows*1..2]-(f:Person) WHERE f.id <> {person}
                   WITH DISTINCT f
                   MATCH (f)-[:hasCreator]->(post:Message)-[:hasTag]->(:Tag {{name:'{seed_tag}'}})
                   WHERE post.isComment = false
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
        # --- Loader-backed tier (task 053) ---
        # IC1: friends within 3 knows hops with the given first name.
        "ic1": f"""MATCH path = (:Person {{id:{person}}})-[:knows* SHORTEST 1..3]-(f:Person)
                   WHERE f.fname = '{first_name}'
                   RETURN length(path) AS dist, f.lname AS lname, f.id AS pid
                   ORDER BY dist, lname, pid LIMIT 20""",
        # IC3: FoF (<=2 hops) foreign to both countries, who posted in both, in window.
        "ic3": f"""MATCH (p:Person {{id:{person}}})-[:knows*1..2]-(f:Person) WHERE f.id <> {person}
                   WITH DISTINCT f
                   WHERE NOT EXISTS {{ MATCH (f)-[:isLocatedIn]->(:Place)-[:isPartOf]->(h:Place) WHERE h.name IN ['China', 'Germany'] }}
                   MATCH (f)-[:hasCreator]->(m:Message)-[:msgCountry]->(c:Place)
                   WHERE c.name IN ['China', 'Germany'] AND m.cdate >= date('{ic3_start}') AND m.cdate < date('{ic3_end}')
                   WITH f, sum(CASE WHEN c.name = 'China' THEN 1 ELSE 0 END) AS xc,
                           sum(CASE WHEN c.name = 'Germany' THEN 1 ELSE 0 END) AS yc
                   WHERE xc > 0 AND yc > 0
                   RETURN f.id AS pid, xc, yc ORDER BY xc + yc DESC, pid ASC LIMIT 20""",
        # IC5: forums the neighbourhood joined after min_date, ranked by member posts.
        "ic5": f"""MATCH (p:Person {{id:{person}}})-[:knows*1..2]-(f:Person) WHERE f.id <> {person}
                   WITH DISTINCT f
                   MATCH (forum:Forum)-[hm:hasMember]->(f) WHERE hm.hd > date('{ic5_min}')
                   MATCH (forum)-[:containerOf]->(post:Message)<-[:hasCreator]-(f)
                   RETURN forum.id AS fid, count(DISTINCT post) AS cnt ORDER BY cnt DESC, fid ASC LIMIT 20""",
        # IC7: latest like per liker of the seed's messages.
        "ic7": f"""MATCH (p:Person {{id:{person}}})-[:hasCreator]->(:Message)<-[lk:likes]-(liker:Person)
                   WITH p, liker, max(lk.ld) AS ld
                   RETURN ld AS ms, liker.id AS lid,
                     CASE WHEN NOT EXISTS {{ MATCH (liker)-[:knows]-(p) }} THEN 1 ELSE 0 END AS isnew
                   ORDER BY ms DESC, lid ASC LIMIT 20""",
        # IC10: foaf (exactly 2 hops) born in the window, scored by interest overlap.
        "ic10": f"""MATCH (p:Person {{id:{person}}})-[:knows]-(:Person)-[:knows]-(foaf:Person)
                    WHERE foaf.id <> {person}
                      AND ((foaf.bmon = 1 AND foaf.bdom >= 21) OR (foaf.bmon = 2 AND foaf.bdom < 22))
                    WITH DISTINCT p, foaf
                    WHERE NOT EXISTS {{ MATCH (p)-[:knows]-(foaf) }}
                    WITH foaf,
                         COUNT {{ MATCH (foaf)-[:hasCreator]->(post:Message) WHERE post.isComment = false }} AS total,
                         COUNT {{ MATCH (foaf)-[:hasCreator]->(post:Message)
                                  WHERE post.isComment = false AND EXISTS {{ MATCH (post)-[:hasTag]->(:Tag)<-[:hasInterest]-(p) }} }} AS common
                    RETURN foaf.id AS pid, 2 * common - total AS score ORDER BY score DESC, pid ASC LIMIT 10""",
        # IC11: neighbourhood working (workFrom<2030) at a company in the seed country.
        "ic11": f"""MATCH (p:Person {{id:{person}}})-[:knows*1..2]-(f:Person) WHERE f.id <> {person}
                    WITH DISTINCT f
                    MATCH (f)-[w:workAt]->(co:Organisation)-[:orgPlace]->(pl:Place)
                    WHERE w.wf < 2030 AND (pl.name = '{seed_country}' OR EXISTS {{ MATCH (pl)-[:isPartOf]->(:Place {{name:'{seed_country}'}}) }})
                    RETURN f.id AS pid, co.name AS cname, w.wf AS wf ORDER BY wf ASC, pid ASC, cname DESC LIMIT 10""",
        # IC12: friends who replied to Posts tagged under the class (or a subclass).
        "ic12": f"""MATCH (:Person {{id:{person}}})-[:knows]-(f:Person)-[:hasCreator]->(c:Message)-[:replyOf]->(post:Message)-[:hasTag]->(:Tag)-[:hasType]->(:TagClass)-[:isSubclassOf*0..10]->(:TagClass {{name:'{seed_class}'}})
                    WHERE post.isComment = false
                    RETURN f.id AS pid, count(DISTINCT c) AS cnt ORDER BY cnt DESC, pid ASC LIMIT 20""",
        # IS1: the seed's profile (first/last name).
        "is1": f"""MATCH (p:Person {{id:{person}}}) RETURN p.fname AS fn, p.lname AS ln""",
        # IC14: weighted shortest-path cost over the 1/(interactions+1) knows graph.
        "ic14": f"""MATCH (a:Person {{id:{person}}}), (b:Person {{id:{person_b}}}),
                    p = (a)-[e:ic14weight * WSHORTEST(w)]->(b)
                    RETURN cost(e) AS dist""",
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
        if name in ("is7", "ic1", "ic3", "ic7"):
            return [[r[0], r[1], r[2]] for r in rows]  # 3-col rows
        if name in ("ic5", "ic10", "ic12", "is1"):
            return [[r[0], r[1]] for r in rows]        # 2-col rows
        if name == "ic11":
            return [[r[0], r[1], r[2]] for r in rows]  # [pid, cname, wf]
        if name == "ic14":
            return [[round(r[0], 6)] for r in rows]    # [cost] (fp, 6 dp)
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
                # compact, canonical separators keep the committed dumps small and
                # byte-stable across re-runs; default=int coerces Kùzu Decimal aggregates.
                json.dump(rows, f, separators=(",", ":"), default=int)
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

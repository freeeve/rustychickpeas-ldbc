#!/usr/bin/env python3
"""Faithful-schema Kùzu side of the LDBC BI head-to-head.

Unlike run.py (a minimal Message-only projection), this loads the full native
LDBC schema rustychickpeas uses — Person (with city location), Place hierarchy
(isPartOf), knows (with creationDate), hasInterest — so the social/location
queries (Q8/Q9/Q11/Q13) can run at Kùzu's best, not just the aggregation ones.

Kùzu still has no label hierarchy, so Post + Comment are projected into one
`Message` table (isComment flag); that is the standard, necessary Kùzu port of
the official `:Message` label, not a simplification of the data.

Preprocessing reads the raw composite-merged-fk CSVs and is NOT timed (only
Kùzu's COPY load + query execution are). Artifacts live under *-faithful so they
never collide with run.py's.

Usage:
  run_faithful.py <initial_snapshot_dir> <scale_label> [--emit-json <dir>]
"""
import csv
import datetime
import glob
import gzip
import os
import shutil
import statistics
import sys
import time

import kuzu

SNAP = sys.argv[1]
SCALE = sys.argv[2] if len(sys.argv) > 2 else "sf"
IMPORT = f"kuzu/import-{SCALE}-faithful"
DB = f"kuzu/db-{SCALE}-faithful"


def rows(subdir, cols):
    """Yield selected columns for every row across the part-*.csv.gz files."""
    base = os.path.join(SNAP, subdir)
    for f in sorted(glob.glob(os.path.join(base, "*.csv.gz"))):
        with gzip.open(f, "rt") as fh:
            r = csv.reader(fh, delimiter="|")
            header = next(r)
            idx = [header.index(c) for c in cols]
            for row in r:
                yield [row[i] for i in idx]


def preprocess():
    """Project the raw LDBC CSVs into Kùzu-friendly comma CSVs (cached)."""
    if os.path.isdir(IMPORT) and os.path.exists(f"{IMPORT}/message.csv"):
        print(f"  (using cached import in {IMPORT})")
        return
    os.makedirs(IMPORT, exist_ok=True)

    # --- Message (Post + Comment) + hasCreator + replyOf + likes + hasTag ---
    with open(f"{IMPORT}/message.csv", "w", newline="") as out, \
         open(f"{IMPORT}/message_hascreator.csv", "w", newline="") as hc:
        w = csv.writer(out, delimiter="|")
        wc = csv.writer(hc, delimiter="|")
        w.writerow(["id", "year", "cdate", "length", "hasContent", "isComment", "lang"])
        wc.writerow(["from", "to"])  # Person -> Message
        for (i, cd, content, length, lang, creator) in rows(
            "dynamic/Post", ["id", "creationDate", "content", "length", "language", "CreatorPersonId"]
        ):
            w.writerow([i, cd[:4], cd[:10], length or 0, "true" if content else "false", "false", lang])
            if creator:
                wc.writerow([creator, i])
        for (i, cd, content, length, creator) in rows(
            "dynamic/Comment", ["id", "creationDate", "content", "length", "CreatorPersonId"]
        ):
            w.writerow([i, cd[:4], cd[:10], length or 0, "true" if content else "false", "true", ""])
            if creator:
                wc.writerow([creator, i])

    with open(f"{IMPORT}/message_replyof.csv", "w", newline="") as out:
        w = csv.writer(out, delimiter="|")
        w.writerow(["from", "to"])  # Comment -> parent Message
        for (i, pp, pc) in rows("dynamic/Comment", ["id", "ParentPostId", "ParentCommentId"]):
            parent = pp if pp else pc
            if parent:
                w.writerow([i, parent])

    with open(f"{IMPORT}/message_likes.csv", "w", newline="") as out:
        w = csv.writer(out, delimiter="|")
        w.writerow(["from", "to"])  # Person -> Message
        for (p, m) in rows("dynamic/Person_likes_Post", ["PersonId", "PostId"]):
            w.writerow([p, m])
        for (p, m) in rows("dynamic/Person_likes_Comment", ["PersonId", "CommentId"]):
            w.writerow([p, m])

    with open(f"{IMPORT}/message_hastag.csv", "w", newline="") as out:
        w = csv.writer(out, delimiter="|")
        w.writerow(["from", "to"])
        for (m, t) in rows("dynamic/Post_hasTag_Tag", ["PostId", "TagId"]):
            w.writerow([m, t])
        for (m, t) in rows("dynamic/Comment_hasTag_Tag", ["CommentId", "TagId"]):
            w.writerow([m, t])

    # --- Person (+ isLocatedIn city as an FK column) ---
    with open(f"{IMPORT}/person.csv", "w", newline="") as out, \
         open(f"{IMPORT}/person_islocatedin.csv", "w", newline="") as li:
        w = csv.writer(out, delimiter="|")
        wl = csv.writer(li, delimiter="|")
        w.writerow(["id", "pcdate", "pym"])  # creationDate + year*12+month, for Q13
        wl.writerow(["from", "to"])  # Person -> Place(city)
        for (i, cd, city) in rows("dynamic/Person", ["id", "creationDate", "LocationCityId"]):
            w.writerow([i, cd[:10], int(cd[:4]) * 12 + int(cd[5:7])])
            if city:
                wl.writerow([i, city])

    # --- Tag / TagClass / hasType ---
    for name, sub, cols in [("tag", "static/Tag", ["id", "name"]),
                            ("tagclass", "static/TagClass", ["id", "name"])]:
        with open(f"{IMPORT}/{name}.csv", "w", newline="") as out:
            w = csv.writer(out, delimiter="|")
            w.writerow(["id", "name"])
            for rec in rows(sub, cols):
                w.writerow(rec)

    with open(f"{IMPORT}/tag_hastype.csv", "w", newline="") as out:
        w = csv.writer(out, delimiter="|")
        w.writerow(["from", "to"])
        for (i, cid) in rows("static/Tag", ["id", "TypeTagClassId"]):
            if cid:
                w.writerow([i, cid])

    # --- Place (+ isPartOf hierarchy as an FK column) ---
    with open(f"{IMPORT}/place.csv", "w", newline="") as out, \
         open(f"{IMPORT}/place_ispartof.csv", "w", newline="") as po:
        w = csv.writer(out, delimiter="|")
        wp = csv.writer(po, delimiter="|")
        w.writerow(["id", "name", "type"])
        wp.writerow(["from", "to"])  # Place -> parent Place
        for (i, nm, typ, parent) in rows("static/Place", ["id", "name", "type", "PartOfPlaceId"]):
            w.writerow([i, nm, typ])
            if parent:
                wp.writerow([i, parent])

    # --- knows (with creationDate) + hasInterest ---
    with open(f"{IMPORT}/knows.csv", "w", newline="") as out:
        w = csv.writer(out, delimiter="|")
        w.writerow(["from", "to", "cdate"])
        for (cd, a, b) in rows("dynamic/Person_knows_Person", ["creationDate", "Person1Id", "Person2Id"]):
            w.writerow([a, b, cd[:10]])

    with open(f"{IMPORT}/hasinterest.csv", "w", newline="") as out:
        w = csv.writer(out, delimiter="|")
        w.writerow(["from", "to"])  # Person -> Tag
        for (p, t) in rows("dynamic/Person_hasInterest_Tag", ["personId", "interestId"]):
            w.writerow([p, t])

    # --- Organisation (Company/University) + workAt, for Q20 ---
    with open(f"{IMPORT}/organisation.csv", "w", newline="") as out:
        w = csv.writer(out, delimiter="|")
        w.writerow(["id", "name", "type"])
        for (i, typ, nm) in rows("static/Organisation", ["id", "type", "name"]):
            w.writerow([i, nm, typ])
    with open(f"{IMPORT}/workat.csv", "w", newline="") as out:
        w = csv.writer(out, delimiter="|")
        w.writerow(["from", "to"])  # Person -> Company
        for (p, c) in rows("dynamic/Person_workAt_Company", ["PersonId", "CompanyId"]):
            w.writerow([p, c])

    # --- Derived weighted edge tables for the weighted-SP queries (Q19/Q20).
    # These replace the per-edge weight closures rust's dijkstra uses: an edge
    # exists only where two people both know each other AND interacted / shared a
    # university, with the derived weight. knows is read once and reused.
    knows_pairs = [(int(a), int(b))
                   for (a, b) in rows("dynamic/Person_knows_Person", ["Person1Id", "Person2Id"])]

    # message -> creator, to count reply interactions between distinct people.
    creator = {}
    for (i, cr) in rows("dynamic/Post", ["id", "CreatorPersonId"]):
        if cr:
            creator[int(i)] = int(cr)
    for (i, cr) in rows("dynamic/Comment", ["id", "CreatorPersonId"]):
        if cr:
            creator[int(i)] = int(cr)
    inter = {}
    for (i, pp, pc) in rows("dynamic/Comment", ["id", "ParentPostId", "ParentCommentId"]):
        a = creator.get(int(i))
        parent = pp if pp else pc
        b = creator.get(int(parent)) if parent else None
        if a and b and a != b:
            k = (a, b) if a < b else (b, a)
            inter[k] = inter.get(k, 0) + 1
    with open(f"{IMPORT}/interactswith.csv", "w", newline="") as out:
        w = csv.writer(out, delimiter="|")
        w.writerow(["from", "to", "w"])
        for (a, b) in knows_pairs:
            k = (a, b) if a < b else (b, a)
            n = inter.get(k, 0)
            if n > 0:
                wt = 1.0 / n
                w.writerow([a, b, wt])
                w.writerow([b, a, wt])  # both directions (knows is undirected)

    # studyAt -> per-person (university, classYear); cohort weight = min|dy|+1.
    study = {}
    for (p, u, cy) in rows("dynamic/Person_studyAt_University", ["PersonId", "UniversityId", "classYear"]):
        study.setdefault(int(p), []).append((int(u), int(cy)))
    with open(f"{IMPORT}/cohort.csv", "w", newline="") as out:
        w = csv.writer(out, delimiter="|")
        w.writerow(["from", "to", "w"])
        for (a, b) in knows_pairs:
            sa, sb = study.get(a), study.get(b)
            if not sa or not sb:
                continue
            best = None
            for (ua, ya) in sa:
                for (ub, yb) in sb:
                    if ua == ub:
                        d = abs(ya - yb)
                        best = d if best is None else min(best, d)
            if best is not None:
                wt = float(best + 1)
                w.writerow([a, b, wt])
                w.writerow([b, a, wt])


def load():
    for p in (DB, DB + ".wal"):
        if os.path.isdir(p):
            shutil.rmtree(p)
        elif os.path.exists(p):
            os.remove(p)
    conn = kuzu.Connection(kuzu.Database(DB))
    ddl = [
        "CREATE NODE TABLE Message(id INT64, year INT64, cdate DATE, length INT64, hasContent BOOLEAN, isComment BOOLEAN, lang STRING, PRIMARY KEY(id))",
        "CREATE NODE TABLE Person(id INT64, pcdate DATE, pym INT64, PRIMARY KEY(id))",
        "CREATE NODE TABLE Tag(id INT64, name STRING, PRIMARY KEY(id))",
        "CREATE NODE TABLE TagClass(id INT64, name STRING, PRIMARY KEY(id))",
        "CREATE NODE TABLE Place(id INT64, name STRING, type STRING, PRIMARY KEY(id))",
        "CREATE REL TABLE hasType(FROM Tag TO TagClass)",
        "CREATE REL TABLE hasTag(FROM Message TO Tag)",
        "CREATE REL TABLE hasCreator(FROM Person TO Message)",
        "CREATE REL TABLE replyOf(FROM Message TO Message)",
        "CREATE REL TABLE likes(FROM Person TO Message)",
        "CREATE REL TABLE knows(FROM Person TO Person, cdate DATE)",
        "CREATE REL TABLE hasInterest(FROM Person TO Tag)",
        "CREATE REL TABLE isLocatedIn(FROM Person TO Place)",
        "CREATE REL TABLE isPartOf(FROM Place TO Place)",
        "CREATE NODE TABLE Organisation(id INT64, name STRING, type STRING, PRIMARY KEY(id))",
        "CREATE REL TABLE workAt(FROM Person TO Organisation)",
        "CREATE REL TABLE interactsWith(FROM Person TO Person, w DOUBLE)",
        "CREATE REL TABLE cohort(FROM Person TO Person, w DOUBLE)",
    ]
    for stmt in ddl:
        conn.execute(stmt)
    t0 = time.time()
    copies = [
        ("Message", "message"), ("Person", "person"), ("Tag", "tag"),
        ("TagClass", "tagclass"), ("Place", "place"),
        ("hasType", "tag_hastype"), ("hasTag", "message_hastag"),
        ("hasCreator", "message_hascreator"), ("replyOf", "message_replyof"),
        ("likes", "message_likes"), ("knows", "knows"),
        ("hasInterest", "hasinterest"), ("isLocatedIn", "person_islocatedin"),
        ("isPartOf", "place_ispartof"), ("Organisation", "organisation"),
        ("workAt", "workat"), ("interactsWith", "interactswith"), ("cohort", "cohort"),
    ]
    for tbl, f in copies:
        conn.execute(f"COPY {tbl} FROM '{IMPORT}/{f}.csv' (HEADER=true, DELIM='|')")
    return conn, time.time() - t0


# ---- Queries (the 6 already validated on the Message schema; Q8/9/11/13 added next) ----
Q1 = """
MATCH (m:Message) WHERE m.cdate < date('2011-12-01') AND m.hasContent = true
RETURN m.year AS year, m.isComment AS isComment,
  CASE WHEN m.length < 40 THEN 0 WHEN m.length < 80 THEN 1 WHEN m.length < 160 THEN 2 ELSE 3 END AS cat,
  count(m) AS cnt, sum(m.length) AS sumLen
ORDER BY year DESC, isComment, cat
"""


def q2_text():
    d0 = datetime.date(2012, 6, 1)
    d1, d2 = d0 + datetime.timedelta(days=100), d0 + datetime.timedelta(days=200)
    return f"""
MATCH (t:Tag)-[:hasType]->(tc:TagClass) WHERE tc.name = 'MusicalArtist'
OPTIONAL MATCH (m1:Message)-[:hasTag]->(t) WHERE m1.cdate >= date('{d0}') AND m1.cdate < date('{d1}')
WITH t, count(m1) AS w1
OPTIONAL MATCH (m2:Message)-[:hasTag]->(t) WHERE m2.cdate >= date('{d1}') AND m2.cdate < date('{d2}')
RETURN t.name AS name, w1, count(m2) AS w2, abs(w1 - count(m2)) AS diff
ORDER BY diff DESC, name LIMIT 100
"""


Q7 = """
MATCH (tag:Tag {name: 'Enrique_Iglesias'})<-[:hasTag]-(message:Message),
      (message)<-[:replyOf]-(comment:Message)-[:hasTag]->(relatedTag:Tag)
WHERE NOT EXISTS { MATCH (comment)-[:hasTag]->(tag) }
RETURN relatedTag.name AS name, count(DISTINCT comment) AS cnt
ORDER BY cnt DESC, name LIMIT 100
"""


def q12_text():
    d0 = datetime.date(2010, 7, 22)
    return f"""
MATCH (person:Person)
OPTIONAL MATCH (person)-[:hasCreator]->(message:Message)-[:replyOf*0..30]->(post:Message)
WHERE post.isComment = false AND message.hasContent = true AND message.length < 20
  AND message.cdate > date('{d0}') AND post.lang IN ['ar', 'hu']
WITH person, count(message) AS messageCount
RETURN messageCount, count(person) AS personCount
ORDER BY personCount DESC, messageCount DESC
"""


Q5 = """
MATCH (tag:Tag {name: 'Abbas_I_of_Persia'})<-[:hasTag]-(message:Message)<-[:hasCreator]-(person:Person)
OPTIONAL MATCH (message)<-[l:likes]-(:Person)
WITH person, message, count(l) AS likeCount
OPTIONAL MATCH (message)<-[:replyOf]-(reply:Message)
WITH person, message, likeCount, count(reply) AS replyCount
WITH person, count(message) AS messageCount, sum(likeCount) AS likeCount, sum(replyCount) AS replyCount
RETURN person.id AS pid, replyCount, likeCount, messageCount,
       messageCount + 2 * replyCount + 10 * likeCount AS score
ORDER BY score DESC, pid LIMIT 100
"""

Q6 = """
MATCH (tag:Tag {name: 'Arnold_Schwarzenegger'})<-[:hasTag]-(message1:Message)<-[:hasCreator]-(person1:Person)
OPTIONAL MATCH (message1)<-[:likes]-(person2:Person)
OPTIONAL MATCH (person2)-[:hasCreator]->(message2:Message)<-[like:likes]-(person3:Person)
RETURN person1.id AS pid, count(DISTINCT like) AS authorityScore
ORDER BY authorityScore DESC, pid LIMIT 100
"""


def q8_text():
    d0, d1 = datetime.date(2011, 7, 20), datetime.date(2011, 7, 25)
    win = f"m.cdate > date('{d0}') AND m.cdate < date('{d1}')"
    return f"""
MATCH (tag:Tag {{name: 'Che_Guevara'}})
MATCH (cand:Person)
WHERE EXISTS {{ MATCH (tag)<-[:hasInterest]-(cand) }}
   OR EXISTS {{ MATCH (tag)<-[:hasTag]-(m:Message)<-[:hasCreator]-(cand) WHERE {win} }}
WITH tag, cand,
  (CASE WHEN EXISTS {{ MATCH (tag)<-[:hasInterest]-(cand) }} THEN 100 ELSE 0 END)
  + COUNT {{ MATCH (tag)<-[:hasTag]-(m:Message)<-[:hasCreator]-(cand) WHERE {win} }} AS score
OPTIONAL MATCH (cand)-[:knows]-(friend:Person)
WITH tag, cand, score, friend,
  CASE WHEN friend IS NULL THEN 0 ELSE
    (CASE WHEN EXISTS {{ MATCH (tag)<-[:hasInterest]-(friend) }} THEN 100 ELSE 0 END)
    + COUNT {{ MATCH (tag)<-[:hasTag]-(m:Message)<-[:hasCreator]-(friend) WHERE {win} }}
  END AS fscore
WITH cand, score, sum(fscore) AS friendsScore
RETURN cand.id AS pid, score, friendsScore
ORDER BY score + friendsScore DESC, pid LIMIT 100
"""


def q9_text():
    d0, d1 = datetime.date(2011, 10, 1), datetime.date(2011, 10, 15)
    return f"""
MATCH (post:Message)<-[:hasCreator]-(person:Person)
WHERE post.isComment = false AND post.cdate >= date('{d0}') AND post.cdate <= date('{d1}')
OPTIONAL MATCH (post)<-[:replyOf*0..30]-(m:Message)
  WHERE m.cdate >= date('{d0}') AND m.cdate <= date('{d1}')
WITH person, post, count(DISTINCT m) AS treeMsgs
WITH person, count(post) AS threads, sum(treeMsgs) AS messages
RETURN person.id AS pid, threads, messages
ORDER BY messages DESC, pid LIMIT 100
"""


def q13_text():
    end = "date('2013-01-01')"
    eym = 2013 * 12 + 1  # end year-month

    def zombie(x, co, mm):
        # France person, created before endDate, < 1 message/month before endDate.
        return (f"{x}.pcdate < {end} "
                f"AND EXISTS {{ MATCH ({x})-[:isLocatedIn]->(:Place)-[:isPartOf]->({co}:Place) "
                f"WHERE {co}.name = 'France' AND {co}.type = 'Country' }} "
                f"AND ({eym} - {x}.pym + 1) > 0 "
                f"AND COUNT {{ MATCH ({x})-[:hasCreator]->({mm}:Message) WHERE {mm}.cdate < {end} }} "
                f"< ({eym} - {x}.pym + 1)")

    # Compute the zombie set ONCE (running the per-person COUNT subquery only for
    # France persons), then score via list membership — re-running the zombie
    # predicate per liker (any active person) is what makes the naive form hang.
    return f"""
MATCH (z0:Person)
WHERE {zombie('z0', 'co', 'mmz')}
WITH collect(z0.id) AS zids
UNWIND zids AS zid
MATCH (z:Person {{id: zid}})
OPTIONAL MATCH (z)-[:hasCreator]->(:Message)<-[:likes]-(liker:Person)
WITH zids, z, liker
WITH z.id AS pid,
  sum(CASE WHEN liker.pcdate < {end} THEN 1 ELSE 0 END) AS tlc,
  sum(CASE WHEN liker.id IN zids THEN 1 ELSE 0 END) AS zlc
RETURN pid, zlc, tlc
ORDER BY (CASE WHEN tlc = 0 THEN 0.0 ELSE zlc * 1.0 / tlc END) DESC, pid LIMIT 100
"""


def q10_text():
    return """
MATCH (s:Person {id: 3470})-[e:knows* SHORTEST 1..4]-(expert:Person)
WHERE length(e) >= 3
MATCH (expert)-[:isLocatedIn]->(:Place)-[:isPartOf]->(:Place {name: 'China', type: 'Country'}),
      (expert)-[:hasCreator]->(message:Message)-[:hasTag]->(:Tag)-[:hasType]->(:TagClass {name: 'MusicalArtist'})
MATCH (message)-[:hasTag]->(tag:Tag)
RETURN expert.id AS eid, tag.name AS tagName, count(DISTINCT message) AS messageCount
ORDER BY messageCount DESC, tagName ASC, eid ASC LIMIT 100
"""


def q16_param(tag, date):
    # Per-param graph work (people who tagged on a date with <=4 such friends);
    # the A-and-B intersection + top-20 is combined in emit_crosscheck.
    return f"""
MATCH (person1:Person)-[:hasCreator]->(message1:Message)-[:hasTag]->(tag:Tag {{name: '{tag}'}})
WHERE message1.cdate = date('{date}')
OPTIONAL MATCH (person1)-[:knows]-(person2:Person)-[:hasCreator]->(message2:Message)-[:hasTag]->(tag)
  WHERE message2.cdate = date('{date}')
WITH person1, count(DISTINCT message1) AS cm, count(DISTINCT person2) AS cp2
WHERE cp2 <= 4
RETURN person1.id AS pid, cm AS cm
"""


def q14_text():
    # HARNESS-REDUCED: Kùzu scores every qualifying knows-pair (the graph work);
    # the per-city best + top-100 reduction is done in emit_crosscheck, since
    # Kùzu 0.11 has no per-group argmax / CALL subqueries / list comprehensions.
    return """
MATCH (country1:Place {name: 'Chile', type: 'Country'})<-[:isPartOf]-(city1:Place)<-[:isLocatedIn]-(person1:Person),
      (person1)-[:knows]-(person2:Person)-[:isLocatedIn]->(:Place)-[:isPartOf]->(:Place {name: 'Argentina', type: 'Country'})
RETURN city1.name AS cityName, person1.id AS p1, person2.id AS p2,
  (CASE WHEN EXISTS { MATCH (person1)-[:hasCreator]->(:Message)-[:replyOf]->(:Message)<-[:hasCreator]-(person2) } THEN 4 ELSE 0 END
   + CASE WHEN EXISTS { MATCH (person1)-[:hasCreator]->(:Message)<-[:replyOf]-(:Message)<-[:hasCreator]-(person2) } THEN 1 ELSE 0 END
   + CASE WHEN EXISTS { MATCH (person1)-[:likes]->(:Message)<-[:hasCreator]-(person2) } THEN 10 ELSE 0 END
   + CASE WHEN EXISTS { MATCH (person1)-[:hasCreator]->(:Message)<-[:likes]-(person2) } THEN 1 ELSE 0 END
  ) AS score
"""


def q18_text():
    return """
MATCH (tag:Tag {name: 'Frank_Sinatra'})<-[:hasInterest]-(person1:Person)-[:knows]-(mutualFriend:Person)-[:knows]-(person2:Person)-[:hasInterest]->(tag)
WHERE person1 <> person2 AND NOT EXISTS { MATCH (person1)-[:knows]-(person2) }
RETURN person1.id AS p1, person2.id AS p2, count(DISTINCT mutualFriend) AS cnt
ORDER BY cnt DESC, p1 ASC, p2 ASC LIMIT 20
"""


def q19_text():
    # Weighted shortest path between people of city 669 and city 648 over the
    # interaction-weighted graph; top-20 pairs by path cost.
    return """
MATCH (a:Person)-[:isLocatedIn]->(:Place {id: 669}),
      (b:Person)-[:isLocatedIn]->(:Place {id: 648}),
      p = (a)-[e:interactsWith * WSHORTEST(w)]->(b)
RETURN a.id AS p1, b.id AS p2, cost(e) AS dist
ORDER BY dist, p1, p2 LIMIT 20
"""


def q20_text():
    # Weighted shortest path from each Falcon_Air employee to person 66 over the
    # university-cohort-weighted graph; top-20 employees by path cost.
    return """
MATCH (company:Organisation {name: 'Falcon_Air', type: 'Company'})<-[:workAt]-(p1:Person),
      (p2:Person {id: 66}),
      path = (p1)-[e:cohort * WSHORTEST(w)]->(p2)
WHERE p1.id <> 66
RETURN p1.id AS pid, cost(e) AS dist
ORDER BY dist, pid LIMIT 20
"""


def q11_text():
    d0, d1 = datetime.date(2012, 9, 29), datetime.date(2013, 1, 1)
    return f"""
MATCH (a:Person)-[k1:knows]-(b:Person)-[k2:knows]-(c:Person)-[k3:knows]-(a:Person)
WHERE a.id < b.id AND b.id < c.id
  AND k1.cdate >= date('{d0}') AND k1.cdate <= date('{d1}')
  AND k2.cdate >= date('{d0}') AND k2.cdate <= date('{d1}')
  AND k3.cdate >= date('{d0}') AND k3.cdate <= date('{d1}')
  AND EXISTS {{ MATCH (a)-[:isLocatedIn]->(:Place)-[:isPartOf]->(ca:Place) WHERE ca.name = 'India' AND ca.type = 'Country' }}
  AND EXISTS {{ MATCH (b)-[:isLocatedIn]->(:Place)-[:isPartOf]->(cb:Place) WHERE cb.name = 'India' AND cb.type = 'Country' }}
  AND EXISTS {{ MATCH (c)-[:isLocatedIn]->(:Place)-[:isPartOf]->(cc:Place) WHERE cc.name = 'India' AND cc.type = 'Country' }}
RETURN count(*) AS cnt
"""


def time_query(conn, name, cypher, runs=5):
    rowcount = len(conn.execute(cypher).get_as_df())
    samples = []
    for _ in range(runs):
        t = time.time()
        conn.execute(cypher).get_as_df()
        samples.append((time.time() - t) * 1000)
    median = statistics.median(samples)
    print(f"  {name:<22} {median:>9.2f} ms   (rows={rowcount})")
    return median


def main():
    print(f"=== Kùzu {kuzu.__version__} — LDBC BI {SCALE} (faithful schema) ===")
    print("Preprocessing raw LDBC CSVs ...")
    t = time.time()
    preprocess()
    print(f"  preprocess: {time.time() - t:.1f}s")

    print("Loading into Kùzu (COPY) ...")
    conn, load_s = load()

    def n(q):
        return conn.execute(q).get_as_df().iloc[0, 0]

    counts = {
        "messages": n("MATCH (m:Message) RETURN count(*)"),
        "persons": n("MATCH (p:Person) RETURN count(*)"),
        "tags": n("MATCH (t:Tag) RETURN count(*)"),
        "tagclasses": n("MATCH (t:TagClass) RETURN count(*)"),
        "places": n("MATCH (p:Place) RETURN count(*)"),
        "knows": n("MATCH ()-[k:knows]->() RETURN count(k)"),
        "hasInterest": n("MATCH ()-[k:hasInterest]->() RETURN count(k)"),
        "isLocatedIn": n("MATCH ()-[k:isLocatedIn]->() RETURN count(k)"),
        "isPartOf": n("MATCH ()-[k:isPartOf]->() RETURN count(k)"),
        "orgs": n("MATCH (o:Organisation) RETURN count(o)"),
        "workAt": n("MATCH ()-[k:workAt]->() RETURN count(k)"),
        "interactsWith": n("MATCH ()-[k:interactsWith]->() RETURN count(k)"),
        "cohort": n("MATCH ()-[k:cohort]->() RETURN count(k)"),
    }
    print(f"  loaded in {load_s:.1f}s: " + ", ".join(f"{k}={v}" for k, v in counts.items()) + "\n")

    if "--emit-json" in sys.argv:
        emit_crosscheck(conn, sys.argv[sys.argv.index("--emit-json") + 1])
        return

    print("Timings (median of 5):")
    time_query(conn, "Q1 posting summary", Q1)
    time_query(conn, "Q2 tag evolution", q2_text())
    time_query(conn, "Q7 related topics", Q7)
    time_query(conn, "Q12 message counts", q12_text(), runs=2)
    time_query(conn, "Q5 active posters", Q5)
    time_query(conn, "Q6 authoritative users", Q6)
    time_query(conn, "Q8 central person", q8_text())
    time_query(conn, "Q9 thread initiators", q9_text())
    time_query(conn, "Q11 friend triangles", q11_text())
    time_query(conn, "Q13 zombies", q13_text())
    time_query(conn, "Q19 interaction path", q19_text(), runs=2)
    time_query(conn, "Q20 recruitment", q20_text(), runs=2)
    time_query(conn, "Q18 friend recommendation", q18_text())
    time_query(conn, "Q14 international dialog", q14_text())


def emit_crosscheck(conn, outdir):
    """Dump query result rows as canonical JSON arrays for the rust cross-check.

    Column order is normalized to match the rust side exactly (compare.py sorts
    rows, so row *order* doesn't matter, but each row's *columns* must align)."""
    import json
    os.makedirs(outdir, exist_ok=True)

    def dump(name, rows):
        with open(f"{outdir}/{name}.kuzu.json", "w") as f:
            json.dump(rows, f)
        return len(rows)

    d = conn.execute(Q1).get_as_df()  # [year, isComment, cat, cnt, sumLen]
    n1 = dump("q1", [[int(y), bool(ic), int(c), int(cnt), int(sl)]
                     for y, ic, c, cnt, sl in zip(d["year"], d["isComment"], d["cat"], d["cnt"], d["sumLen"])])
    d = conn.execute(q2_text()).get_as_df()  # [name, w1, w2, diff]
    n2 = dump("q2", [[str(nm), int(w1), int(w2), int(x)]
                     for nm, w1, w2, x in zip(d["name"], d["w1"], d["w2"], d["diff"])])
    d = conn.execute(Q5).get_as_df()  # [pid, messageCount, replyCount, likeCount, score]
    n5 = dump("q5", [[int(p), int(mc), int(rc), int(lc), int(sc)]
                     for p, rc, lc, mc, sc in zip(d["pid"], d["replyCount"], d["likeCount"], d["messageCount"], d["score"])])
    d = conn.execute(Q6).get_as_df()  # [pid, score]
    n6 = dump("q6", [[int(p), int(a)] for p, a in zip(d["pid"], d["authorityScore"])])
    d = conn.execute(Q7).get_as_df()  # [name, count]
    n7 = dump("q7", [[str(nm), int(c)] for nm, c in zip(d["name"], d["cnt"])])
    d = conn.execute(q12_text()).get_as_df()  # [messageCount, personCount]
    n12 = dump("q12", [[int(mc), int(pc)] for mc, pc in zip(d["messageCount"], d["personCount"])])
    d = conn.execute(q8_text()).get_as_df()  # [pid, score, friendsScore]
    n8 = dump("q8", [[int(p), int(s), int(f)] for p, s, f in zip(d["pid"], d["score"], d["friendsScore"])])
    d = conn.execute(q9_text()).get_as_df()  # [pid, threads, messages]
    n9 = dump("q9", [[int(p), int(t), int(m)] for p, t, m in zip(d["pid"], d["threads"], d["messages"])])
    d = conn.execute(q11_text()).get_as_df()  # [[count]]
    n11 = int(d["cnt"].iloc[0])
    dump("q11", [[n11]])
    d = conn.execute(q13_text()).get_as_df()  # [pid, zlc, tlc]
    n13 = dump("q13", [[int(p), int(z), int(t)] for p, z, t in zip(d["pid"], d["zlc"], d["tlc"])])
    d = conn.execute(q19_text()).get_as_df()  # [p1, p2, dist] (cost rounded to 6dp)
    n19 = dump("q19", [[int(a), int(b), round(float(x), 6)] for a, b, x in zip(d["p1"], d["p2"], d["dist"])])
    d = conn.execute(q20_text()).get_as_df()  # [pid, dist]
    n20 = dump("q20", [[int(p), round(float(x), 6)] for p, x in zip(d["pid"], d["dist"])])
    d = conn.execute(q18_text()).get_as_df()  # [p1, p2, mutualCount]
    n18 = dump("q18", [[int(a), int(b), int(c)] for a, b, c in zip(d["p1"], d["p2"], d["cnt"])])
    d = conn.execute(q14_text()).get_as_df()  # scored pairs; per-city best + top100 below
    best14 = {}
    for cn, p1, p2, sc in zip(d["cityName"], d["p1"], d["p2"], d["score"]):
        cn, p1, p2, sc = str(cn), int(p1), int(p2), int(sc)
        key = (-sc, p1, p2)  # minimize: max score, then min p1, then min p2
        if cn not in best14 or key < best14[cn][0]:
            best14[cn] = (key, [p1, p2, cn, sc])
    rows14 = sorted((v[1] for v in best14.values()), key=lambda r: (-r[3], r[0], r[1]))[:100]
    n14 = dump("q14", rows14)
    da = conn.execute(q16_param("Meryl_Streep", "2012-09-16")).get_as_df()
    db_ = conn.execute(q16_param("Hank_Williams", "2012-05-08")).get_as_df()
    ra = {int(p): int(c) for p, c in zip(da["pid"], da["cm"])}
    rb = {int(p): int(c) for p, c in zip(db_["pid"], db_["cm"])}
    dump("q16a", sorted([p, c] for p, c in ra.items()))
    dump("q16b", sorted([p, c] for p, c in rb.items()))
    rows16 = sorted(([p, ra[p], rb[p]] for p in ra.keys() & rb.keys()),
                    key=lambda r: (-(r[1] + r[2]), r[0]))[:20]
    n16 = dump("q16", rows16)
    d = conn.execute(q10_text()).get_as_df()  # [eid, tagName, messageCount]
    n10 = dump("q10", [[int(e), str(t), int(c)] for e, t, c in zip(d["eid"], d["tagName"], d["messageCount"])])

    print(f"  emitted faithful-Kùzu cross-check JSON to {outdir} "
          f"(q1={n1}, q2={n2}, q5={n5}, q6={n6}, q7={n7}, q8={n8}, q9={n9}, "
          f"q10={n10}, q11={n11}, q12={n12}, q13={n13}, q14={n14}, q16={n16}, q18={n18}, q19={n19}, q20={n20})")


if __name__ == "__main__":
    main()

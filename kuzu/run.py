#!/usr/bin/env python3
"""Reference-engine side of the LDBC BI head-to-head: load the same SFn data
into Kùzu and run the faithful Q1/Q2 queries, for comparison against the
rustychickpeas `ldbc-bench` binary on the same machine and scale.

Kùzu has no label hierarchy (no `:Message` supertype), so we preprocess Post +
Comment into one `Message` node table (with an `isComment` flag) and the two
`*_hasTag_*` files into one `hasTag` rel table. Preprocessing reads the same raw
LDBC CSVs; it is NOT counted in either engine's timings (only Kùzu's COPY load
and query execution are timed). Dates are kept as ISO strings — LDBC creationDate
sorts chronologically as text, so range filters are plain string comparisons.

Usage: run.py <initial_snapshot_dir> <scale_label>   e.g. run.py .../initial_snapshot sf10
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
IMPORT = f"kuzu/import-{SCALE}"
DB = f"kuzu/db-{SCALE}"


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

    with open(f"{IMPORT}/message.csv", "w", newline="") as out, \
         open(f"{IMPORT}/message_hascreator.csv", "w", newline="") as hc:
        w = csv.writer(out)
        wc = csv.writer(hc)
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

    with open(f"{IMPORT}/person.csv", "w", newline="") as out:
        w = csv.writer(out)
        w.writerow(["id"])
        for (i,) in rows("dynamic/Person", ["id"]):
            w.writerow([i])

    with open(f"{IMPORT}/message_replyof.csv", "w", newline="") as out:
        w = csv.writer(out)
        w.writerow(["from", "to"])  # Comment -> parent Message
        for (i, pp, pc) in rows("dynamic/Comment", ["id", "ParentPostId", "ParentCommentId"]):
            parent = pp if pp else pc
            if parent:
                w.writerow([i, parent])

    with open(f"{IMPORT}/message_likes.csv", "w", newline="") as out:
        w = csv.writer(out)
        w.writerow(["from", "to"])  # Person -> Message
        for (p, m) in rows("dynamic/Person_likes_Post", ["PersonId", "PostId"]):
            w.writerow([p, m])
        for (p, m) in rows("dynamic/Person_likes_Comment", ["PersonId", "CommentId"]):
            w.writerow([p, m])

    for name, sub, cols in [("tag", "static/Tag", ["id", "name"]),
                            ("tagclass", "static/TagClass", ["id", "name"])]:
        with open(f"{IMPORT}/{name}.csv", "w", newline="") as out:
            w = csv.writer(out)
            w.writerow(["id", "name"])
            for rec in rows(sub, cols):
                w.writerow(rec)

    with open(f"{IMPORT}/tag_hastype.csv", "w", newline="") as out:
        w = csv.writer(out)
        w.writerow(["from", "to"])
        for (i, cid) in rows("static/Tag", ["id", "TypeTagClassId"]):
            if cid:
                w.writerow([i, cid])

    with open(f"{IMPORT}/message_hastag.csv", "w", newline="") as out:
        w = csv.writer(out)
        w.writerow(["from", "to"])
        for (m, t) in rows("dynamic/Post_hasTag_Tag", ["PostId", "TagId"]):
            w.writerow([m, t])
        for (m, t) in rows("dynamic/Comment_hasTag_Tag", ["CommentId", "TagId"]):
            w.writerow([m, t])


def load():
    # Kùzu may store the DB as a file or a directory (plus a .wal); clear both.
    for p in (DB, DB + ".wal"):
        if os.path.isdir(p):
            shutil.rmtree(p)
        elif os.path.exists(p):
            os.remove(p)
    conn = kuzu.Connection(kuzu.Database(DB))
    conn.execute("CREATE NODE TABLE Message(id INT64, year INT64, cdate DATE, length INT64, hasContent BOOLEAN, isComment BOOLEAN, lang STRING, PRIMARY KEY(id))")
    conn.execute("CREATE NODE TABLE Tag(id INT64, name STRING, PRIMARY KEY(id))")
    conn.execute("CREATE NODE TABLE TagClass(id INT64, name STRING, PRIMARY KEY(id))")
    conn.execute("CREATE NODE TABLE Person(id INT64, PRIMARY KEY(id))")
    conn.execute("CREATE REL TABLE hasType(FROM Tag TO TagClass)")
    conn.execute("CREATE REL TABLE hasTag(FROM Message TO Tag)")
    conn.execute("CREATE REL TABLE hasCreator(FROM Person TO Message)")
    conn.execute("CREATE REL TABLE replyOf(FROM Message TO Message)")
    conn.execute("CREATE REL TABLE likes(FROM Person TO Message)")
    t0 = time.time()
    for tbl, f in [("Message", "message"), ("Tag", "tag"), ("TagClass", "tagclass"),
                   ("Person", "person"), ("hasType", "tag_hastype"), ("hasTag", "message_hastag"),
                   ("hasCreator", "message_hascreator"), ("replyOf", "message_replyof"),
                   ("likes", "message_likes")]:
        conn.execute(f'COPY {tbl} FROM "{IMPORT}/{f}.csv" (HEADER=true)')
    return conn, time.time() - t0


# Grouped scan + aggregation (the dominant work). The official query's
# percentage-of-total is a trivial post-division and is omitted from the timed
# query to keep both engines doing the same grouped aggregation.
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


def time_query(conn, name, cypher, runs=5):
    rowcount = len(conn.execute(cypher).get_as_df())  # warmup + result
    samples = []
    for _ in range(runs):
        t = time.time()
        res = conn.execute(cypher)
        _ = res.get_as_df()
        samples.append((time.time() - t) * 1000)
    median = statistics.median(samples)
    print(f"  {name:<22} {median:>9.2f} ms   (rows={rowcount})")
    return median


def emit_crosscheck(conn, outdir):
    """Dump Q1/Q2 result rows as canonical JSON arrays for the rust cross-check."""
    import json
    os.makedirs(outdir, exist_ok=True)
    d1 = conn.execute(Q1).get_as_df()
    r1 = [[int(y), bool(ic), int(c), int(cnt), int(sl)]
          for y, ic, c, cnt, sl in zip(d1["year"], d1["isComment"], d1["cat"], d1["cnt"], d1["sumLen"])]
    with open(f"{outdir}/q1.kuzu.json", "w") as f:
        json.dump(r1, f)
    d2 = conn.execute(q2_text()).get_as_df()
    r2 = [[str(n), int(w1), int(w2), int(d)]
          for n, w1, w2, d in zip(d2["name"], d2["w1"], d2["w2"], d2["diff"])]
    with open(f"{outdir}/q2.kuzu.json", "w") as f:
        json.dump(r2, f)
    print(f"  emitted Q1/Q2 Kùzu cross-check JSON to {outdir} (q1={len(r1)} rows, q2={len(r2)} rows)")


def main():
    print(f"=== Kùzu {kuzu.__version__} — LDBC BI {SCALE} ===")
    print("Preprocessing raw LDBC CSVs ...")
    t = time.time()
    preprocess()
    print(f"  preprocess: {time.time() - t:.1f}s")

    print("Loading into Kùzu (COPY) ...")
    conn, load_s = load()
    n_msg = conn.execute("MATCH (m:Message) RETURN count(*)").get_as_df().iloc[0, 0]
    n_tag = conn.execute("MATCH (t:Tag) RETURN count(*)").get_as_df().iloc[0, 0]
    print(f"  loaded {n_msg} messages, {n_tag} tags in {load_s:.1f}s\n")

    if "--emit-json" in sys.argv:
        emit_crosscheck(conn, sys.argv[sys.argv.index("--emit-json") + 1])
        return

    print(f"Timings (median of 5):")
    time_query(conn, "Q1 posting summary", Q1)
    time_query(conn, "Q2 tag evolution", q2_text())
    time_query(conn, "Q7 related topics", Q7)
    # Q12's naive recursive-path translation is very slow in Kùzu; fewer runs.
    time_query(conn, "Q12 message counts", q12_text(), runs=2)
    time_query(conn, "Q5 active posters", Q5)
    time_query(conn, "Q6 authoritative users", Q6)


if __name__ == "__main__":
    main()

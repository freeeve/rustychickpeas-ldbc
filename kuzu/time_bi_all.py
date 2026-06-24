#!/usr/bin/env python3
"""Time ALL 20 LDBC BI queries on Kùzu in one warm process (median of 5).

Every formulation here is the fair one already cross-checked value-identical vs
rustychickpeas (see run_faithful.emit_crosscheck + kuzu/compare.py):

  * single-Cypher          : Q1 Q2 Q5 Q6 Q7 Q8 Q10 Q11 Q15 Q18 Q19 Q20
  * reply-forest (WCC+numpy): Q3 Q4 Q9 Q12 Q17 — PROJECT_GRAPH + WEAKLY_CONNECTED_COMPONENTS
                              once, then component->root-post lookups (NOT replyOf*0..30)
  * weighted shortest path : Q15 Q19 Q20 via WSHORTEST
  * harness-reduced        : Q13 Q14 Q16 — Kùzu does the graph work, Python does
                              the per-group argmax / set-intersection that Kùzu 0.11
                              Cypher can't express

Reply-forest queries report (a) = projection + WCC + reduce (the forest is shared
across Q3/Q4/Q12 — it is built once and the ~WCC cost is the same one-time pass).

Usage:  .venv-kuzu/bin/python kuzu/time_bi_all.py   # needs kuzu/db-sf1-faithful
"""
import os
import statistics
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
if len(sys.argv) < 3:
    sys.argv = [sys.argv[0], "/tmp", "sf1"]
import numpy as np  # noqa: E402
import kuzu  # noqa: E402
import run_faithful as rf  # noqa: E402
import time_q12_fair as q12f  # noqa: E402

DB = os.path.join(HERE, "db-sf1-faithful")
RUNS = 5
database = kuzu.Database(DB)
conn = kuzu.Connection(database)

# Tuned, value-identical Q6 (DISTINCT (person1,person2) before the 2-hop like expansion).
Q6_FAIR = """
MATCH (tag:Tag {name: 'Arnold_Schwarzenegger'})<-[:hasTag]-(message1:Message)<-[:hasCreator]-(person1:Person),
      (message1)<-[:likes]-(person2:Person)
WITH DISTINCT person1, person2
MATCH (person2)-[:hasCreator]->(:Message)<-[like:likes]-(:Person)
WITH person1, count(like) AS auth
RETURN person1.id AS pid, sum(auth) AS authorityScore
ORDER BY authorityScore DESC, pid LIMIT 100
"""


def median(fn, runs=RUNS):
    res, s = None, []
    for _ in range(runs):
        t = time.time()
        res = fn()
        s.append((time.time() - t) * 1000)
    return statistics.median(s), res


# --- reply-forest: projection + WCC once, shared by Q3/Q4/Q12 -----------------
conn.execute(q12f.PROJECT)
TOTAL = int(conn.execute(rf.q12_person_count()).get_as_df()["cnt"].iloc[0])
m_proj, _ = median(lambda: kuzu.Connection(database).execute(q12f.PROJECT))
wcc_s = []
compmap = None
for _ in range(RUNS):
    t = time.time()
    compmap = q12f._wcc_compmap(conn)
    wcc_s.append((time.time() - t) * 1000)
m_wcc = statistics.median(wcc_s)
mid_all = compmap["mid"].to_numpy()
comp_all = compmap["comp"].to_numpy()
_order = np.argsort(mid_all)
mid_sorted = mid_all[_order]
comp_sorted = comp_all[_order]
FOREST_MS = m_proj + m_wcc   # shared one-time reply-forest cost


def comp_of(mids):
    """Thread component for each message id (all ids are WCC-labelled)."""
    idx = np.minimum(np.searchsorted(mid_sorted, mids), len(mid_sorted) - 1)
    return comp_sorted[idx]


def _in_list(ids):
    return "[" + ",".join(str(int(i)) for i in ids) + "]"


# comp -> root-post's containerOf forum (each thread component has one root Post that
# a Forum is the containerOf). Shared by Q9 / Q17 (replaces their replyOf*0..30 walk
# to the thread root + forum).
_cf = conn.execute(
    "MATCH (forum:Forum)-[:containerOf]->(post:Message) RETURN forum.id AS fid, post.id AS mid"
).get_as_df()
COMP2FORUM = dict(zip(comp_of(_cf["mid"].to_numpy()).tolist(), (int(x) for x in _cf["fid"])))


# --- Q3: musical-artist messages in a country's forums' reply-trees ----------
Q3_TAGGED = ("MATCH (m:Message)-[:hasTag]->(:Tag)-[:hasType]->(:TagClass {name:'MusicalArtist'}) "
             "RETURN DISTINCT m.id AS mid")


def q3_reduce():
    dform = conn.execute(rf.q3_forums()).get_as_df()
    if len(dform) == 0:
        return 0
    fids = [int(x) for x in dform["fid"]]
    roots = conn.execute(
        f"MATCH (forum:Forum)-[:containerOf]->(post:Message) WHERE forum.id IN {_in_list(fids)} "
        "RETURN forum.id AS fid, post.id AS mid").get_as_df()
    comp2forum = dict(zip(comp_of(roots["mid"].to_numpy()).tolist(),
                          (int(x) for x in roots["fid"])))
    tcomp = comp_of(conn.execute(Q3_TAGGED).get_as_df()["mid"].to_numpy())
    fcount = {}
    for c in tcomp.tolist():
        fid = comp2forum.get(c)
        if fid is not None:
            fcount[fid] = fcount.get(fid, 0) + 1
    rows = sorted(([f, cnt] for f, cnt in fcount.items()), key=lambda r: (-r[1], r[0]))[:20]
    return len(rows)


# --- Q4: top forums, then rank their members by reply-tree message count ------
Q4_MSGCREATOR = "MATCH (m:Message)<-[:hasCreator]-(p:Person) RETURN m.id AS mid, p.id AS pid"


def q4_reduce():
    dfm = conn.execute(rf.q4_forum_members()).get_as_df()
    ranked = sorted(zip((int(x) for x in dfm["fid"]), (int(x) for x in dfm["numberOfMembers"]),
                        (int(x) for x in dfm["cid"])), key=lambda r: (-r[1], r[0], r[2]))
    top_ids, seen = [], set()
    for fid, _, _ in ranked:
        if fid not in seen:
            seen.add(fid)
            top_ids.append(fid)
            if len(top_ids) == 100:
                break
    roots = conn.execute(
        f"MATCH (forum:Forum)-[:containerOf]->(post:Message) WHERE forum.id IN {_in_list(top_ids)} "
        "RETURN post.id AS mid").get_as_df()["mid"].to_numpy()
    top_comps = np.unique(comp_of(roots))
    mc_df = conn.execute(Q4_MSGCREATOR).get_as_df()
    keep = np.isin(comp_of(mc_df["mid"].to_numpy()), top_comps)
    uniq, cnts = np.unique(mc_df["pid"].to_numpy()[keep], return_counts=True)
    mc = dict(zip(uniq.tolist(), cnts.tolist()))
    members = [int(p) for p in conn.execute(rf.q4_members(top_ids)).get_as_df()["pid"]]
    rows = sorted(([p, mc.get(p, 0)] for p in members), key=lambda r: (-r[1], r[0]))[:100]
    return len(rows)


# --- Q13: zombies + per-zombie liker aggregation ------------------------------
def q13_reduce():
    zset = {int(x) for x in conn.execute(rf.q13_zids()).get_as_df()["z"]}
    agg = {z: [0, 0] for z in zset}
    ld = conn.execute(rf.q13_likes()).get_as_df()
    for z, l, active in zip(ld["z"], ld["l"], ld["active"]):
        a = agg[int(z)]
        a[1] += int(active)
        if int(l) in zset:
            a[0] += 1
    rows = sorted(([z, agg[z][0], agg[z][1]] for z in zset),
                  key=lambda r: (-(r[1] / r[2] if r[2] else 0.0), r[0]))[:100]
    return len(rows)


# --- Q14: scored knows-pairs, per-city argmax + top100 ------------------------
def q14_reduce():
    d = conn.execute(rf.q14_text()).get_as_df()
    best = {}
    for cn, p1, p2, sc in zip(d["cityName"], d["p1"], d["p2"], d["score"]):
        cn, p1, p2, sc = str(cn), int(p1), int(p2), int(sc)
        key = (-sc, p1, p2)
        if cn not in best or key < best[cn][0]:
            best[cn] = (key, [p1, p2, cn, sc])
    rows = sorted((v[1] for v in best.values()), key=lambda r: (-r[3], r[0], r[1]))[:100]
    return len(rows)


# --- Q16: two per-param graph passes, intersection + top20 --------------------
def q16_reduce():
    da = conn.execute(rf.q16_param("Meryl_Streep", "2012-09-16")).get_as_df()
    db_ = conn.execute(rf.q16_param("Hank_Williams", "2012-05-08")).get_as_df()
    ra = {int(p): int(c) for p, c in zip(da["pid"], da["cm"])}
    rb = {int(p): int(c) for p, c in zip(db_["pid"], db_["cm"])}
    rows = sorted(([p, ra[p], rb[p]] for p in ra.keys() & rb.keys()),
                  key=lambda r: (-(r[1] + r[2]), r[0]))[:20]
    return len(rows)


# --- Q9 (WCC): per person, posts in a window + window messages in those posts'
# reply trees. Component membership replaces (post)<-[:replyOf*0..30]-(m). ----------
def q9_reduce():
    d0, d1 = "2011-10-01", "2011-10-15"
    roots = conn.execute(
        f"MATCH (post:Message)<-[:hasCreator]-(person:Person) "
        f"WHERE post.isComment=false AND post.cdate>=date('{d0}') AND post.cdate<=date('{d1}') "
        f"RETURN post.id AS mid, person.id AS pid").get_as_df()
    rcomp = comp_of(roots["mid"].to_numpy())
    wm = conn.execute(
        f"MATCH (m:Message) WHERE m.cdate>=date('{d0}') AND m.cdate<=date('{d1}') "
        f"RETURN m.id AS mid").get_as_df()["mid"].to_numpy()
    uniq, cnts = np.unique(comp_of(wm), return_counts=True)
    wcount = dict(zip(uniq.tolist(), cnts.tolist()))
    threads, messages = {}, {}
    for pid, c in zip((int(x) for x in roots["pid"]), rcomp.tolist()):
        threads[pid] = threads.get(pid, 0) + 1
        messages[pid] = messages.get(pid, 0) + wcount.get(c, 0)
    rows = sorted(([p, threads[p], messages[p]] for p in threads),
                  key=lambda r: (-r[2], r[0]))[:100]
    return len(rows)


# --- Q17 (WCC): m1 tuples + candidates + memberships, nested propagation join.
# The thread root / forum (f1, f2) come from the component map instead of
# (m1)-[:replyOf*0..30]->(post)<-[:containerOf]-(f) — the only replyOf left is the
# single-hop (comment)-[:replyOf]->(m2), which is not recursive. ----------
def q17_reduce():
    T = rf.TAG17
    m1q = conn.execute(
        f"MATCH (p1:Person)-[:hasCreator]->(m1:Message)-[:hasTag]->(:Tag {{name:'{T}'}}) "
        f"RETURN DISTINCT p1.id AS p1, m1.id AS m1, m1.mts AS ms1").get_as_df()
    m1c = comp_of(m1q["m1"].to_numpy())
    m1_set = set()
    for p1, ms1, c in zip((int(x) for x in m1q["p1"]), (int(x) for x in m1q["ms1"]), m1c.tolist()):
        f1 = COMP2FORUM.get(c)
        if f1 is not None:
            m1_set.add((p1, f1, ms1))
    m1_list = list(m1_set)
    cdf = conn.execute(
        f"""MATCH (p2:Person)-[:hasCreator]->(comment:Message)-[:replyOf]->(m2:Message),
                  (p3:Person)-[:hasCreator]->(m2)
            WHERE comment.isComment = true
              AND EXISTS {{ MATCH (comment)-[:hasTag]->(:Tag {{name:'{T}'}}) }}
              AND EXISTS {{ MATCH (m2)-[:hasTag]->(:Tag {{name:'{T}'}}) }}
            RETURN DISTINCT p2.id AS p2, p3.id AS p3, m2.id AS m2, m2.mts AS ms2""").get_as_df()
    m2c = comp_of(cdf["m2"].to_numpy())
    cand = []
    for p2, p3, m2, ms2, c in zip((int(x) for x in cdf["p2"]), (int(x) for x in cdf["p3"]),
                                  (int(x) for x in cdf["m2"]), (int(x) for x in cdf["ms2"]), m2c.tolist()):
        f2 = COMP2FORUM.get(c)
        if f2 is not None:
            cand.append((p2, p3, m2, f2, ms2))
    involved = ({p for p, _, _ in m1_list} | {p2 for p2, _, _, _, _ in cand}
                | {p3 for _, p3, _, _, _ in cand})
    pm = {}
    if involved:
        memdf = conn.execute(rf.q17_mem(sorted(involved))).get_as_df()
        for p, f in zip((int(x) for x in memdf["p"]), (int(x) for x in memdf["f"])):
            pm.setdefault(p, set()).add(f)
    delta = 4 * 3600000
    counts = {}
    for p2, p3, m2, f2, ms2 in cand:
        if p2 == p3:
            continue
        fp2, fp3 = pm.get(p2, set()), pm.get(p3, set())
        for p1, f1, ms1 in m1_list:
            if f1 != f2 and ms2 > ms1 + delta and f1 in fp2 and f1 in fp3 and f2 not in pm.get(p1, set()):
                counts.setdefault(p1, set()).add(m2)
    rows = sorted(([p, len(ms)] for p, ms in counts.items()), key=lambda r: (-r[1], r[0]))[:10]
    return len(rows)


def n_rows(cypher):
    return lambda: len(conn.execute(cypher).get_as_df())


def q11_value():
    return int(conn.execute(rf.q11_text()).get_as_df()["cnt"].iloc[0])


def q16_value_count():  # Q16 yields 0 rows on the official params
    return q16_reduce()


# (name, fn, kind, expected_result_label, is_reply_forest)
SPEC = [
    ("Q1 posting summary", n_rows(rf.Q1), "12 groups", False),
    ("Q2 tag evolution", n_rows(rf.q2_text()), "100 tags", False),
    ("Q3 popular topics", q3_reduce, "20", True),
    ("Q4 top creators", q4_reduce, "100", True),
    ("Q5 active posters", n_rows(rf.Q5), "100", False),
    ("Q6 authoritative users", n_rows(Q6_FAIR), "100", False),
    ("Q7 related topics", n_rows(rf.Q7), "100", False),
    ("Q8 central person", n_rows(rf.q8_text()), "100", False),
    ("Q9 thread initiators", q9_reduce, "100", True),
    ("Q10 experts in country", n_rows(rf.q10_text()), "100", False),
    ("Q11 friend triangles", q11_value, "805 triangles", False),
    ("Q12 message histogram", None, "86 buckets", True),  # via q12_fair
    ("Q13 zombies", q13_reduce, "5", False),
    ("Q14 international dialog", q14_reduce, "7", False),
    ("Q15 weighted path", n_rows(rf.q15_text()), "1", False),
    ("Q16 fake news", q16_reduce, "0", False),
    ("Q17 information propagation", q17_reduce, "10", True),
    ("Q18 friend recommendation", n_rows(rf.q18_text()), "20", False),
    ("Q19 interaction path", n_rows(rf.q19_text()), "6 pairs", False),
    ("Q20 recruitment", n_rows(rf.q20_text()), "1", False),
]

# rustychickpeas faithful Q1-Q20, SF1, from docs/bench-bi.md
RUST = {
    "Q1": 2.8, "Q2": 6.5, "Q3": 1.8, "Q4": 56.0, "Q5": 0.4, "Q6": 137.0, "Q7": 2.2,
    "Q8": 0.4, "Q9": 9.2, "Q10": 9.9, "Q11": 3.8, "Q12": 5.1, "Q13": 0.2, "Q14": 6.8,
    "Q15": 18.0, "Q16": 0.2, "Q17": 2.0, "Q18": 113.0, "Q19": 7.1, "Q20": 0.3,
}


def main():
    try:
        load = os.getloadavg()
    except OSError:
        load = (0.0, 0.0, 0.0)
    print(f"=== Kùzu {kuzu.__version__}  LDBC BI ALL-20 fair head-to-head  "
          f"(db-sf1-faithful, median of {RUNS}) ===")
    print(f"    loadavg(1/5/15m) = {load[0]:.2f}/{load[1]:.2f}/{load[2]:.2f}")
    print(f"    shared reply-forest one-time: projection {m_proj:.1f} ms + WCC {m_wcc:.1f} ms "
          f"= {FOREST_MS:.1f} ms (Q3/Q4/Q12)\n")
    q12 = q12f.q12_fair(database, runs=RUNS)
    rows, all_ok = [], True
    for name, fn, expect, forest in SPEC:
        qid = name.split()[0]
        if qid == "Q12":
            ms, got = q12["a_ms"], q12["buckets"]
            res = f"{got} buckets"
            ok = got == 86
        else:
            ms, got = median(fn)
            if forest:
                ms = FOREST_MS + ms
            ok = str(got) == expect.split()[0]
            res = expect if ok else f"{got} (exp {expect})"
        all_ok &= ok
        rust = RUST[qid]
        winner = "Kùzu" if ms < rust else "rustychickpeas"
        flag = "  <== Kùzu WINS" if ms < rust else ""
        rows.append((name, rust, ms, res, ok, winner, flag))

    w = max(len(r[0]) for r in rows)
    print(f"  {'query':<{w}}  {'rusty_ms':>9}  {'kuzu_ms':>10}  {'result':<14} {'winner':<14} parity")
    print(f"  {'-'*w}  {'-'*9}  {'-'*10}  {'-'*14} {'-'*14} ------")
    for name, rust, ms, res, ok, winner, flag in rows:
        print(f"  {name:<{w}}  {rust:>9.1f}  {ms:>10.2f}  {res:<14} {winner:<14} "
              f"{'OK' if ok else 'FAIL'}{flag}")
    wins = [r[0] for r in rows if r[5] == "Kùzu"]
    print(f"\n  Kùzu wins ({len(wins)}): {', '.join(w.split()[0] for w in wins) or 'none'}")
    print(f"  PARITY (all 20 cross-checked value-identical earlier): "
          f"{'ALL result counts OK' if all_ok else 'COUNT MISMATCH'}")


if __name__ == "__main__":
    main()

#!/usr/bin/env python3
"""Time the faithful Kùzu BI queries end-to-end (median of N), reporting the
Kùzu-execute vs pandas-reduction split for the five harness-reduced queries.

Usage: time_queries.py <snapshot> <subdir> [runs]
Reuses the query strings + reduction logic from run_faithful so the timings
match exactly what the cross-check certified.
"""
import sys
import time
import statistics

import kuzu

SNAP = sys.argv[1]
SUB = sys.argv[2] if len(sys.argv) > 2 else "sf1"
RUNS = int(sys.argv[3]) if len(sys.argv) > 3 else 5

# run_faithful reads argv at import time; give it what it expects.
sys.argv = ["run_faithful.py", SNAP, SUB]
sys.path.insert(0, "kuzu")
import run_faithful as rf  # noqa: E402

conn = kuzu.Connection(kuzu.Database(f"kuzu/db-{SUB}-faithful"))


def kx(text):
    return conn.execute(text).get_as_df()


# One-time projected graph for Q4 (WCC); measured separately as setup cost.
_t = time.perf_counter()
conn.execute("CALL PROJECT_GRAPH('rg', ['Message'], ['replyOf'])")
PROJECT_MS = (time.perf_counter() - _t) * 1000.0


def pure(text):
    """A query whose entire cost is the Cypher execute (no pandas reduction)."""
    def run():
        t = time.perf_counter()
        df = kx(text)
        return (time.perf_counter() - t) * 1000.0, 0.0, len(df)
    return run


def q13():
    t = time.perf_counter()
    zset = {int(x) for x in kx(rf.q13_zids())["z"]}
    ld = kx(rf.q13_likes())
    kms = (time.perf_counter() - t) * 1000.0
    t = time.perf_counter()
    agg = {z: [0, 0] for z in zset}
    for z, l, active in zip(ld["z"], ld["l"], ld["active"]):
        a = agg[int(z)]
        a[1] += int(active)
        if int(l) in zset:
            a[0] += 1
    rows = sorted(([z, agg[z][0], agg[z][1]] for z in zset),
                  key=lambda r: (-(r[1] / r[2] if r[2] else 0.0), r[0]))[:100]
    return kms, (time.perf_counter() - t) * 1000.0, len(rows)


def q14():
    t = time.perf_counter()
    d = kx(rf.q14_text())
    kms = (time.perf_counter() - t) * 1000.0
    t = time.perf_counter()
    best = {}
    for cn, p1, p2, sc in zip(d["cityName"], d["p1"], d["p2"], d["score"]):
        cn, p1, p2, sc = str(cn), int(p1), int(p2), int(sc)
        key = (-sc, p1, p2)
        if cn not in best or key < best[cn][0]:
            best[cn] = (key, [p1, p2, cn, sc])
    rows = sorted((v[1] for v in best.values()), key=lambda r: (-r[3], r[0], r[1]))[:100]
    return kms, (time.perf_counter() - t) * 1000.0, len(rows)


def q16():
    t = time.perf_counter()
    da = kx(rf.q16_param("Meryl_Streep", "2012-09-16"))
    db_ = kx(rf.q16_param("Hank_Williams", "2012-05-08"))
    kms = (time.perf_counter() - t) * 1000.0
    t = time.perf_counter()
    ra = {int(p): int(c) for p, c in zip(da["pid"], da["cm"])}
    rb = {int(p): int(c) for p, c in zip(db_["pid"], db_["cm"])}
    rows = sorted(([p, ra[p], rb[p]] for p in ra.keys() & rb.keys()),
                  key=lambda r: (-(r[1] + r[2]), r[0]))[:20]
    return kms, (time.perf_counter() - t) * 1000.0, len(rows)


def q17():
    t = time.perf_counter()
    m1df = kx(rf.q17_m1())
    m1_list = list(zip((int(x) for x in m1df["p1"]), (int(x) for x in m1df["f1"]),
                       (int(x) for x in m1df["ms1"])))
    cdf = kx(rf.q17_cand())
    cand = list(zip((int(x) for x in cdf["p2"]), (int(x) for x in cdf["p3"]),
                    (int(x) for x in cdf["m2"]), (int(x) for x in cdf["f2"]),
                    (int(x) for x in cdf["ms2"])))
    involved = ({p for p, _, _ in m1_list} | {p2 for p2, _, _, _, _ in cand}
                | {p3 for _, p3, _, _, _ in cand})
    pm = {}
    if involved:
        memdf = kx(rf.q17_mem(sorted(involved)))
        for p, f in zip((int(x) for x in memdf["p"]), (int(x) for x in memdf["f"])):
            pm.setdefault(p, set()).add(f)
    kms = (time.perf_counter() - t) * 1000.0
    t = time.perf_counter()
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
    return kms, (time.perf_counter() - t) * 1000.0, len(rows)


def q12():
    t = time.perf_counter()
    rld = kx(rf.q12_root_lang())
    rootlang = {int(c): (str(l) if l is not None else None) for c, l in zip(rld["comp"], rld["lang"])}
    dmc = kx(rf.q12_msg_comp())
    total = int(kx(rf.q12_person_count())["cnt"].iloc[0])
    kms = (time.perf_counter() - t) * 1000.0
    t = time.perf_counter()
    langset = {"ar", "hu"}
    permsg = {}
    for comp, pid in zip(dmc["comp"], dmc["pid"]):
        if rootlang.get(int(comp)) in langset:
            permsg[int(pid)] = permsg.get(int(pid), 0) + 1
    hist = {}
    for c in permsg.values():
        hist[c] = hist.get(c, 0) + 1
    hist[0] = hist.get(0, 0) + (total - len(permsg))
    rows = sorted(([mc, pc] for mc, pc in hist.items()), key=lambda r: (-r[1], -r[0]))
    return kms, (time.perf_counter() - t) * 1000.0, len(rows)


def q3():
    t = time.perf_counter()
    dform = kx(rf.q3_forums())
    finfo = {int(f): (str(tt), fc, int(p)) for f, tt, fc, p in
             zip(dform["fid"], dform["title"], dform["fcdate"], dform["pid"])}
    cfd = kx(rf.q3_comp_forum(list(finfo)))
    comp2forum = {int(c): int(f) for c, f in zip(cfd["comp"], cfd["fid"])}
    tcd = kx(rf.q3_tagged_comp())
    kms = (time.perf_counter() - t) * 1000.0
    t = time.perf_counter()
    fcount = {}
    for comp, cnt in zip(tcd["comp"], tcd["cnt"]):
        fid = comp2forum.get(int(comp))
        if fid is not None:
            fcount[fid] = fcount.get(fid, 0) + int(cnt)
    rows = sorted(([f, finfo[f][0], rf.fday_of(finfo[f][1]), finfo[f][2], c]
                   for f, c in fcount.items()), key=lambda r: (-r[4], r[0]))[:20]
    return kms, (time.perf_counter() - t) * 1000.0, len(rows)


def q4():
    t = time.perf_counter()
    dfm = kx(rf.q4_forum_members())
    ranked = sorted(zip((int(x) for x in dfm["fid"]), (int(x) for x in dfm["numberOfMembers"]),
                        (int(x) for x in dfm["cid"])), key=lambda r: (-r[1], r[0], r[2]))
    top_ids, seen = [], set()
    for fid, _, _ in ranked:
        if fid not in seen:
            seen.add(fid)
            top_ids.append(fid)
            if len(top_ids) == 100:
                break
    top_comps = {int(x) for x in kx(rf.q4_wcc_topcomps(top_ids))["comp"]}
    dmc = kx(rf.q4_msg_comp())
    members = [int(p) for p in kx(rf.q4_members(top_ids))["pid"]]
    kms = (time.perf_counter() - t) * 1000.0
    t = time.perf_counter()
    keep = dmc[dmc["comp"].isin(top_comps)]
    mc = {int(p): int(n) for p, n in keep.groupby("pid").size().items()}
    rows = sorted(([p, mc.get(p, 0)] for p in members), key=lambda r: (-r[1], r[0]))[:100]
    return kms, (time.perf_counter() - t) * 1000.0, len(rows)


# Faithful BI order; harness-reduced ones flagged with a callable instead of text.
QUERIES = [
    ("q1", pure(rf.Q1), False), ("q2", pure(rf.q2_text()), False),
    ("q3", q3, True), ("q4", q4, True),
    ("q5", pure(rf.Q5), False), ("q6", pure(rf.Q6), False),
    ("q7", pure(rf.Q7), False), ("q8", pure(rf.q8_text()), False),
    ("q9", pure(rf.q9_text()), False), ("q10", pure(rf.q10_text()), False),
    ("q11", pure(rf.q11_text()), False), ("q12", q12, True),
    ("q13", q13, True), ("q14", q14, True),
    ("q15", pure(rf.q15_text()), False), ("q16", q16, True),
    ("q17", q17, True), ("q18", pure(rf.q18_text()), False),
    ("q19", pure(rf.q19_text()), False), ("q20", pure(rf.q20_text()), False),
]

import os  # noqa: E402
_only = os.environ.get("LDBC_ONLY")
if _only:
    keep = set(_only.split(","))
    QUERIES = [q for q in QUERIES if q[0] in keep]

print(f"# Kùzu faithful timings ({SUB}, median of {RUNS}); PROJECT_GRAPH setup = {PROJECT_MS:.1f} ms")
print(f"# {'query':<6} {'total_ms':>10} {'kuzu_ms':>10} {'pandas_ms':>10} {'rows':>7}  kind")
for name, fn, reduced in QUERIES:
    fn()  # warmup
    totals, kmss, pmss = [], [], []
    nrows = 0
    for _ in range(RUNS):
        kms, pms, nrows = fn()
        totals.append(kms + pms)
        kmss.append(kms)
        pmss.append(pms)
    kind = "reduced" if reduced else "pure"
    print(f"  {name:<6} {statistics.median(totals):>10.2f} {statistics.median(kmss):>10.2f} "
          f"{statistics.median(pmss):>10.2f} {nrows:>7}  {kind}")

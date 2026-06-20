# 053 — Extend the faithful Kùzu import; cross-check the loader-backed IC tier — DONE

Extended `kuzu/run_faithful.py`'s `preprocess()` + schema + COPY with the
IC-only rels/properties (all additive):
- Person `fname`/`lname` (IC1/IS1) and birthday `bmon`/`bdom` (IC10)
- `msgCountry` Message->Place rels (IC3)
- `hasMember.hd` join date (IC5); `likes.ld` like time (IC7)
- `workAt.wf` workFrom + `orgPlace` Organisation->Place (IC11)
- `isSubclassOf` TagClass hierarchy (IC12)

Rebuilt `db-sf1-faithful` from the regenerated import (2.86M messages, ~48s).
The BI faithful cross-check is unaffected: **20/20 q1–q20 still byte-identical**
on the rebuilt DB.

Added the 8 queries' Cypher to `kuzu/run_ic.py` + comparable projections; the
`ic` binary's emit now also writes the loader-backed projections (with
`seed_country`/`seed_class` in seeds.json). Two Kùzu-dialect fixes: IC10's
`*2..2` + `EXISTS`-in-aggregate segfaulted / mis-scored, rewritten as an
explicit 2-hop with two `OPTIONAL MATCH`es and `2*common - total`; and Decimal
aggregates coerced on json.dump.

Result: `compare.py` over **all 19 cross-checkable IC queries** (ic1–ic13,
is1/2/3/5/6/7) → **ALL PASS** byte-identical on SF1. rustychickpeas wins every
one but IC13 (ties); biggest gaps IC4 ~300x, IC10 ~150x, IC7 ~55x. Timings in
`results/ic-sf1.txt` + README.

Not cross-checked: IC14 (task 054) and IS4 (content text kept out of the shared
faithful import to keep BI loads lean).

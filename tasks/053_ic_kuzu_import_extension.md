# 053 — Extend the faithful Kùzu import; cross-check the loader-backed IC tier

**Goal.** Add the IC-only edges/properties to the faithful Kùzu import and
rebuild `db-sf1-faithful`, then cross-check IC1, IC3, IC5, IC7, IC10, IC11, IC12
and IS1, IS4 — the queries our rust loader added schema for (commits cfd15e4 /
960534c) that the faithful projection lacks.

**Why.** These are validated rust-side only. A like-for-like Kùzu reference needs
the same edges/properties loaded.

**Depends on.** 004, 052, and the rust loader additions: `msgCountry`,
`hasMember.hd`, `likes.ld`, Person `fname`/`lname`/`gender`/birthday,
`workAt.wf` + Organisation location, `TagClass isSubclassOf`, message `ctext`.

**Files.**
- `kuzu/run_faithful.py` — extend `preprocess()` + the schema DDL / COPY list:
  Person firstName/lastName/gender/birthday; Message content text + a
  LocationCountryId edge; Forum_hasMember creationDate; likes creationDate;
  workAt workFrom + Organisation LocationPlaceId; TagClass SubclassOfTagClassId.
- `kuzu/run_ic.py` — add the 9 queries' Cypher + emit.
- Rebuild via `run_faithful.py <snapshot> sf1` (regenerates the import + DB).

**Steps.**
1. Mirror the rust loader's additive schema in the faithful import.
2. Translate IC1/3/5/7/10/11/12 + IS1/4 Cypher with the seeds pick_seeds chose
   (Person.id = plid; IS1/IC1 now have firstName/lastName).
3. Emit comparable projections; `compare.py`.
4. Re-run the BI faithful cross-check to confirm the import additions are
   additive (no BI regression).

**Acceptance.**
- The 9 queries byte-identical rust vs Kùzu on SF1.
- BI faithful cross-check unaffected by the import additions.
- Results folded into `results/ic-sf1.txt` + README.

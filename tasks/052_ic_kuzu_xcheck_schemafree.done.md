# 052 — IC Kùzu cross-check: schema-compatible tier (no rebuild) — DONE

Extended the IC head-to-head to IC4, IC6, IC8, IS6, IS7 on the existing
`db-sf1-faithful` (no rebuild).

- `src/interactive.rs`: hoisted the seed_tag / seed_post / IC4-window derivations
  above the `LDBC_EMIT_JSON` branch; the emit now also writes comparable
  projections — IC4/IC6 `[tagName, count]`, IC8 `[ms]`, IS6 `[forumFlid,
  moderatorPlid]`, IS7 `[ms, authorPlid, knows]` — and adds `seed_tag` + the IC4
  window to `seeds.json` so Kùzu runs identical params. IS6/IS7 anchor on "the
  seed's newest Post", which both engines derive the same way (max mts).
- `kuzu/run_ic.py`: added the five queries' Cypher (IC4 uses a `NOT EXISTS`
  never-before-tag filter) + the projection cases.

Result: `compare.py` over ic2 ic4 ic6 ic8 ic9 ic13 is2 is3 is5 is6 is7 → **ALL
PASS** byte-identical on SF1. Timings folded into `results/ic-sf1.txt` + README:
rustychickpeas wins every cross-checkable query but IC13 (IC4 ~300x faster —
Kùzu's NOT-EXISTS subquery is pathologically slow).

Next: 053 (faithful-import rebuild for the loader-backed tier), 054 (IC14).

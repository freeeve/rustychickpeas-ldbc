# 004 — Interactive (IC) Kùzu reference side

**Goal.** Run the official IC Cypher on Kùzu over the same snapshot, for the
laptop head-to-head — the IC analogue of `kuzu/run_faithful.py`.

**Why.** A local same-machine, same-data comparison is the project's honest
substitute for unavailable single-node SF1 published numbers (see README "Can we
compare to other systems?").

**Depends on.** 003 (so result shapes are known to validate against).

**Files.**
- new `kuzu/run_ic.py` (mirror `run_faithful.py`: COPY-load the projected
  Post+Comment `Message` table, run IC Cypher, time only load + execution)
- official Cypher: `ldbc/ldbc_snb_interactive_v2_impls` (neo4j `queries/`)

**Steps.**
1. Reuse the existing Kùzu import for the shared snapshot; add any IC-only edges
   IC needs that the BI projection skipped.
2. Translate the IC Cypher templates with the same seeds `pick_seeds` chose so
   both engines run identical parameters.
3. Time load + per-query execution; emit a comparison table like the BI one.

**Acceptance.**
- Each IC query returns the same result shape on Kùzu and rustychickpeas
  (correctness validated before quoting magnitudes).
- A `rustychickpeas vs Kùzu` IC table lands in `results/` and the README.

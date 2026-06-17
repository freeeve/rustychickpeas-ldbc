# 078 — FinBench → Kùzu import (reference DB)

**Goal.** Load the FinBench `raw/` CSV into a Kùzu database so the 12 complex
reads can be cross-checked + timed in Cypher (head-to-head with the Rust impls).

**Why.** The existing `kuzu/` infra (`compare.py`, `db-sf1`, `import-*`) is the
SNB schema. FinBench is a different schema — needs its own node/rel tables + a
COPY-based bulk load, mirroring the SNB import pattern and the graphalytics Kùzu
references (tasks 128–133).

**Depends on.** 007 (the generated `data/finbench/raw`).

**Steps.**
1. Kùzu schema DDL for the FinBench tables:
   - nodes: `Account`, `Person`, `Company`, `Medium`, `Loan` (per-type i64 ids).
   - rels: `transfer`, `withdraw`, `deposit`, `repay`, `signIn`, `guarantee`
     (person/company), `apply` (person/company → loan), `own`, `invest` — each
     with `createTime` + (where applicable) `amount` as rel properties, so Cypher
     can filter on time/amount mid-pattern.
2. `COPY` each entity/edge CSV (pipe-delimited, header row) into its table.
3. A `kuzu/finbench_import.py` (or extend `import-*`) producing `kuzu/db-finbench-sf{1,10}`.
4. Print node/edge counts; sanity-check against the Rust loader's counts
   (110,547 / 881,805 at SF1).

**Acceptance.**
- `kuzu/db-finbench-sf1` (and `-sf10`) built from `raw/`; counts match the Rust loader.
- A trivial Cypher (`MATCH (a:Account) RETURN count(a)`) runs and matches.

**Status: pending.** Prerequisite for the Kùzu phase of tasks 079–090.

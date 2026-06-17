# 078 — FinBench → Kùzu import (reference DB)

**Goal.** Load the FinBench `raw/` CSV into a Kùzu database so the 12 complex
reads can be cross-checked + timed in Cypher (head-to-head with the Rust impls).

## Done

`kuzu/finbench_import.py` — trims each pipe-delimited CSV to the query-relevant
columns (read with `QUOTE_NONE`, only the early columns, which sit before the
free-text `comment` field) and `COPY`s into a minimal Kùzu schema matching the
Rust loader's node/edge set:

- nodes: `Account`(id, isBlocked), `Person`, `Company`, `Loan`(id, loanAmount,
  balance), `Medium`(id, isBlocked) — i64 primary keys per type.
- rels: `transfer`/`withdraw`/`deposit`/`repay` (createTime + amount),
  `signIn`, `person/companyGuarantee`, `person/companyApply` (+ loanAmount),
  `person/companyOwn`, `person/companyInvest` (createTime).

Run: `.venv-kuzu/bin/python kuzu/finbench_import.py [raw_dir] [db_path]`.

## Acceptance — met
- `kuzu/db-finbench-sf10` built from `data/finbench/raw` in 61.8 s.
- **Counts match the Rust loader exactly: 1,103,805 nodes / 8,962,710 edges.**
- Cypher runs: `MATCH (a:Account) RETURN count(a)` = 439,171; a windowed/amount
  filter (`transfer.amount > 1e6`) returns 1,643,629. `kuzu/db-*` is gitignored.

**Status: done.** Prerequisite for the Kùzu phase of tasks 079–090.

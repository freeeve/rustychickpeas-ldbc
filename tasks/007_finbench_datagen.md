# 007 — FinBench data generation + loader

**Goal.** Generate a small FinBench dataset and load its transaction schema into
rustychickpeas.

**Why.** FinBench is the Tier-2 breadth play — a financial transaction network,
different from SNB. Heaviest lift of the set: new schema + a Spark-based
generator, so it is scoped separately from the queries (`tasks/008`).

**Depends on.** 001.

**Files.**
- `scripts/gen_finbench.sh` (created; clones + builds `ldbc_finbench_datagen`)
- new loader in the lib (FinBench CSV layout differs from SNB BI)
- `src/bin/finbench.rs` + the `finbench` module

**Schema (nodes / edges).**
- nodes: Account, Person, Company, Medium, Loan
- edges (time-stamped, amount-weighted): `transfer`, `withdraw`, `deposit`,
  `repay`, `guarantee`, `invest`, `signIn`, `own`, `apply`

**Steps.**
1. Generate the smallest scale factor with `gen_finbench.sh` (needs Spark/Java —
   document the prerequisite; this is not a curl-and-go like BI).
2. Write the loader: per-type id maps (FinBench ids, like SNB, are i64 unique
   only within a type), edge timestamps + amounts as edge properties so the
   relationship accessor's `pos` can read them during traversal (the Q11
   capability).
3. Print node/edge counts and load time, matching the BI loader's report.

**Acceptance.**
- Smallest-SF FinBench loads; counts and load time printed.
- Edge timestamp + amount readable during traversal via the relationship
  accessor.

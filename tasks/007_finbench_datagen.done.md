# 007 — FinBench data generation + loader

**Goal.** Generate a small FinBench dataset and load its transaction schema into
rustychickpeas.

## Done

### Datagen (`scripts/gen_finbench.sh`)
Reproducible recipe (commit `f6018b6`): clones `ldbc_finbench_datagen`, builds the
shaded jar (`mvn package`), and runs it directly with `java` in Spark local mode —
the jar bundles Spark and hardcodes a `local` master, so **no spark-submit and no
separate Spark install**, only a JDK (Homebrew `openjdk@11` works) plus the Java-11
`--add-opens` flags. `./scripts/gen_finbench.sh 1 data/finbench` → pipe-delimited
CSV under `data/finbench/raw`. `/.cache/` (the 120 MB clone) is gitignored.

### Loader (`src/finbench.rs::load_finbench` + `src/bin/finbench.rs`)
- Plain-CSV `for_each_csv` (FinBench CSV isn't gzipped), columns resolved by header.
- Per-type i64 -> NodeId maps (Account/Person/Company/Loan/Medium are unique only
  within a type). Nodes get labels + key props (account `blocked`, loan `amount`/
  `balance`); rels resolve endpoints through the right maps.
- Rels carry `ts` (createTime) + `amt` (amount) as **rel properties** via
  `set_relationship_props_by_index`, so the queries (`tasks/008`) read them during
  traversal through the relationship accessor's `pos`.
- Covers transfer/withdraw/deposit/repay/apply (amount) and
  guarantee/own/invest/signIn (timestamp).

## Acceptance — met
- **SF1 loads:** `110,547 nodes, 881,805 rels in 883 ms` (counts + load time printed).
- **Rel ts + amount readable during traversal:** `sample transfer 0 -> 1:
  ts=I64(1636994977131) amt=8581508.92`.

**Status: done.** Queries are `tasks/008`.

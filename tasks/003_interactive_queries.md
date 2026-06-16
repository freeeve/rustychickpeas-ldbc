# 003 — Interactive (IC) queries

**Goal.** Implement the IC read workload as hand-coded traversals in
`src/bin/ic.rs`, reusing the BI capabilities, and time them with `time_query`.

**Why.** IC is the seed-anchored traversal shape we already win (cf. Q7/Q12). It
turns the head-to-head into "Kùzu wins analytical scans (BI), we win interactive
traversals (IC)" — both halves measured.

**Depends on.** 001, 002. Stubs in `src/interactive.rs`.

**Scope — feasible with the currently-loaded schema:**
- IC1 friends-by-name (≤3-hop `knows` BFS), IC2/IC9 recent (FoF) messages,
  IC13 unweighted shortest path, IC14 weighted shortest path (reuse Q19/Q20
  dijkstra + interaction-weight map), short reads IS1–IS3/IS5 (profile, message,
  content, replies).

**Scope — needs more schema loaded first (note, defer if costly):**
- IC4/IC5 (Forum `hasMember`/membership dates), IC6 (tag co-occurrence),
  IC10/IC11/IC12 (Organisation `workAt`/`studyAt`, tagclass expert search). Some
  of these reuse loaders BI already added (Q20 `workAt`/`studyAt`, Q10 experts).

**Steps.**
1. Fill the `ic*` stubs against `pick_seeds` output.
2. For each, assert the result shape (count/non-empty) as a smoke test, mirroring
   the BI "surfaces real artists" checks.
3. Register them in the `ic` binary's `time_query` block; print a results table.

**Acceptance.**
- All Tier-feasible IC queries run on SF1 and produce stable, non-empty results.
- Per-query median-of-5 timings printed; deferred IC queries listed explicitly
  (no silent omission).

# 002 — Interactive (IC) query seeds

**Goal.** Produce the per-query seed parameters IC needs (a start person, dates,
names, person pairs) **without a new download** — derive them from the
`initial_snapshot` we already load.

**Why.** IC queries are parameterized (substitution parameters). The official
driver ships factor-table-derived parameters, but at laptop scale we can pick
representative, reproducible seeds straight from the loaded graph — the same way
the BI queries use the official example parameters. This keeps IC zero-download.

**Depends on.** 001.

**Files.**
- new `src/bin/ic.rs` (later; this task just adds a `seeds` helper to the lib or
  the ic binary)
- optional `results/ic-seeds.json` — captured seeds for reproducibility

**Steps.**
1. Add a `pick_seeds(&GraphSnapshot)` helper: a well-connected start Person (max
   `knows` degree), a date window that contains messages, a common first name
   for IC1, and two reachable Persons for IC13/IC14.
2. Make seed selection deterministic (sort by id / degree, take the first) so
   runs are comparable; emit them via `emit_json` for the record.
3. Document in `README` that IC reuses the BI snapshot — no `download_ic.sh`.

**Acceptance.**
- `pick_seeds` returns a populated seed set on SF1 and SF10.
- Seeds are stable across runs and printed/emitted for reproducibility.

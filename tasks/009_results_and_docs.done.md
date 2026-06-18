# 009 — Results tables + README/docs for the new families

**Goal.** Fold IC, Graphalytics, FinBench (and SPB if done) into the README with
per-family results tables and the same honesty caveats as the BI section.

**Why.** The README's value is its honest framing ("magnitudes preliminary",
"can we compare?"). New families must inherit that discipline.

**Depends on.** 003/004 (IC), 006 (Graphalytics), 008 (FinBench), 010 + 013 (SPB,
incl. the FTS/geo core features 011/012).

**Steps.**
1. Add a section per family with: what loads, what runs, SF1 (and SF10 where
   feasible) timings, and the validation status.
2. Graphalytics: report PASS/FAIL against reference outputs — the one family
   with real validation; lead with that.
3. IC / FinBench: same "correctness validated, magnitudes preliminary on a loaded
   machine, single-threaded vs Kùzu multi-threaded" caveat as BI.
4. Update the roadmap and `docs/families.md` sequencing checkboxes.

**Acceptance.**
- README has a results table per completed family.
- Every quoted number carries its caveat; validated vs timing-only is explicit.

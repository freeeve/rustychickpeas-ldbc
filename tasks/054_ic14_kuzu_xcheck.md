# 054 — IC14 weighted-path Kùzu cross-check

**Goal.** Cross-check IC14 (weighted interaction shortest path) rust vs Kùzu.

**Why.** IC14 was deferred on the Kùzu side in 004 — its edge weight is a derived
interaction count (`1 / (interactions + 1)`), not a stored property, so it needs
a weight projection both engines compute identically.

**Depends on.** 004. The faithful `interactsWith(w)` edge may already carry a
usable weight (built for BI Q19).

**Files.**
- `kuzu/run_ic.py` — IC14 weighted shortest path (Kùzu `WSHORTEST`, or a
  projected weight edge matching `build_knows_interaction`).
- `src/interactive.rs` — emit IC14 (path cost + length) comparably.

**Steps.**
1. Confirm the shared weight: rust `build_knows_interaction` = reply interactions
   between message creators. Check whether faithful `interactsWith(w)` matches or
   needs its own projection.
2. Emit IC14 cost (+ path length). Path node ids are not comparable across
   engines (internal vs LDBC), so compare the cost (fp tolerance) + length.

**Acceptance.**
- IC14 path cost matches rust vs Kùzu on SF1 within fp tolerance, or the
  divergence is explained.

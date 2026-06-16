# 004 — Interactive (IC) Kùzu reference side — DONE

`kuzu/run_ic.py` runs the feasible IC tier against the existing
`db-sf1-faithful` (read-only) with the same seeds `pick_seeds` chose,
mirroring `run_faithful.py`. The rust `ic` binary gained an `LDBC_EMIT_JSON`
mode emitting comparable projections (LDBC ids / `ms` timestamps, never
internal node ids); the shared `kuzu/compare.py` diffs the two sides.

Value-level cross-check on SF1 — **ALL PASS** (byte-identical row sets):
ic2 (20), ic9 (20), ic13 (1), is2 (10), is3 (848), is5 (1).

Head-to-head timings (median of 5) in `results/ic-sf1.txt` and the README:
rustychickpeas wins the multi-hop traversals (IC2 ~3x, IC9 ~4x) and the
sub-ms short reads (IS2/IS3); Kùzu's native path engine is competitive on the
single IC13 shortest path.

Deferred on the Kùzu side: IC1/IS1 (the faithful Person projection omits
firstName/lastName — would need an import rebuild) and IC14 (interaction-weight
semantics). The cross-check covers every query feasible on the shared faithful
Message/Person/knows/hasCreator/replyOf projection.

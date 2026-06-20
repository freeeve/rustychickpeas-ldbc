# 054 — IC14 weighted-path Kùzu cross-check — DONE

The faithful `interactsWith(w)` rel carries `1/n` (Q19, n>0 only), not IC14's
`1/(interactions+1)` over *every* knows rel — so added a dedicated
`ic14weight(FROM Person TO Person, w DOUBLE)` table to `run_faithful.py`'s
preprocess (reusing the already-computed `inter` reply-interaction dict; both
knows directions) + DDL + COPY, and rebuilt `db-sf1-faithful`. BI is unaffected
(still 20/20 q1–q20 identical on the rebuild).

`kuzu/run_ic.py` IC14 uses `(a)-[e:ic14weight * WSHORTEST(w)]->(b) RETURN cost(e)`
(mirroring the Q19/Q20 weighted-path form); the `ic` binary emits the path cost
rounded to 6 dp (path node ids aren't comparable across engines). On SF1 both
engines report cost **1.148053** → `compare.py` ic14 PASS.

Timings: rust 19.4 ms (dijkstra with a per-rel interaction-weight lookup) vs
Kùzu 7.1 ms (native WSHORTEST over the precomputed rel) — Kùzu wins, as on
IC13.

This closes the IC↔Kùzu cross-check: **all 20 cross-checkable IC queries
(IC1–IC14, IS1/2/3/5/6/7) are byte-identical**. Only IS4 (content text) stays
rust-side by design.

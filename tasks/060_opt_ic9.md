# 060 — Optimize IC9 (recent FoF messages)

chickpeas 231 ms / Kùzu 1672 ms (slow on both).

Both walk the <=2-hop neighbourhood then its messages, top-20 by date.
chickpeas: bfs_distances(<=2) then per-person hasCreator scan. Kùzu:
`knows*1..2` + DISTINCT message. Look for a cheaper neighbourhood materialize
+ message top-k on each side.

Timings are SF1 under load avg ~6.7 (read relatively; re-measure on a quiet machine). Acceptance: faster on the targeted engine AND still byte-identical via kuzu/compare.py.

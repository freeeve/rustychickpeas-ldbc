# 059 — Optimize IC6 (tag co-occurrence)

chickpeas 43 ms / Kùzu 2301 ms.

Kùzu: `knows*1..2` to the neighbourhood then per-post co-tag DISTINCT counts.
Try materializing the <=2-hop neighbour set once, or restructuring the
co-occurrence aggregation.

Timings are SF1 under load avg ~6.7 (read relatively; re-measure on a quiet machine). Acceptance: faster on the targeted engine AND still byte-identical via kuzu/compare.py.

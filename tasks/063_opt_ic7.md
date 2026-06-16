# 063 — Optimize IC7 (recent likers)

chickpeas 2 ms / Kùzu 228 ms.

Kùzu: max(ld) per liker over the seed's messages' incoming likes + a knows
EXISTS. Check whether the per-liker aggregate or the knows test dominates and
restructure.

Timings are SF1 under load avg ~6.7 (read relatively; re-measure on a quiet machine). Acceptance: faster on the targeted engine AND still byte-identical via kuzu/compare.py.

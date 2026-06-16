# 064 — Optimize IS7 (replies of a message)

chickpeas 0.03 ms / Kùzu 150 ms.

Kùzu: replies of the seed's newest post + per-reply author + a knows EXISTS.
Small absolute cost but a large ratio; check the per-reply knows test.

Timings are SF1 under load avg ~6.7 (read relatively; re-measure on a quiet machine). Acceptance: faster on the targeted engine AND still byte-identical via kuzu/compare.py.

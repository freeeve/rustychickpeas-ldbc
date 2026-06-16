# 062 — Optimize IC3 (friends in two countries)

chickpeas 174 ms / Kùzu 745 ms (slow on both).

Both scan the <=2-hop neighbourhood's messages filtered by msgCountry + window.
Look for earlier pruning (country/window) and a cheaper per-person count on each
side.

Timings are SF1 under load avg ~6.7 (read relatively; re-measure on a quiet machine). Acceptance: faster on the targeted engine AND still byte-identical via kuzu/compare.py.

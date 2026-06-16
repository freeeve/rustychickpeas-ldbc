# 058 — Optimize IC5 (new groups)

chickpeas 999 ms / Kùzu 4455 ms (slow on both).

chickpeas: per-FoF incoming hasMember scan + per-forum containerOf post scan;
consider hoisting/iterating forums once. Kùzu: collect(members) per forum then a
post scan with `cr IN members`; reformulate the membership join.

Timings are SF1 under load avg ~6.7 (read relatively; re-measure on a quiet machine). Acceptance: faster on the targeted engine AND still byte-identical via kuzu/compare.py.

# 055 — Optimize IC4 (new topics) — DONE

Kùzu IC4: 6347 ms -> 133 ms (~48x). Fix: group the in-window posts per tag
FIRST (count(DISTINCT post)), then run the never-before `NOT EXISTS` check once
per distinct in-window tag instead of per (post,tag) row. A collect()+`IN`-list
variant was tried and was worse (Kùzu lists are linear-scanned). Still
byte-identical (compare.py ic4 PASS).

The head-to-head gap drops from ~670x to ~15x (chickpeas ~9 ms) — most of the
original gap was the naive correlated subquery, not the engine.

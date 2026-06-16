# 063 — Optimize IC7 (recent likers) — DONE (reviewed, no change)

Kùzu IC7 ~228 ms. Reviewed: no IN-list or per-row-subquery anti-pattern to fix.
The cost is the per-liker max(ld) aggregation over the seed's messages' incoming
likes; the knows EXISTS runs once per already-grouped liker. Small absolute, left
as-is. chickpeas wins via CSR + a small HashMap (~2 ms).

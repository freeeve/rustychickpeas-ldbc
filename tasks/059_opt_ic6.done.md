# ic6 — Optimize IC6 (tag co-occurrence) — DONE

Kùzu: 2300 ms -> 217 (~10x) ms. Dedup the <=2-hop neighbourhood (WITH DISTINCT f) before scanning posts, instead of re-scanning per knows path. Byte-identical (compare.py PASS).

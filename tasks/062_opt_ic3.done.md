# ic3 — Optimize IC3 (two countries) — DONE

Kùzu: 745 ms -> 375 (~2x) ms. Move the home-country NOT-EXISTS after WITH DISTINCT f so it runs per distinct foaf, not per knows path. Byte-identical (compare.py PASS).

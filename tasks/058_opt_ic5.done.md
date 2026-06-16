# ic5 — Optimize IC5 (new groups) — DONE

Kùzu: 4455 ms -> 410 (~11x) ms. Dropped collect(members)+'cr IN members' (Kuzu lists are linear-scanned); bind the qualifying member and the post creator as the same f structurally. Byte-identical (compare.py PASS).

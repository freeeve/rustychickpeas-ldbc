# 071 — Optimize SPB a14

Baseline (full set, median of 5): 8.5 ms, 17 allocs, on the SPB-10 extract.
Lead: CPU: webDocumentType edge traversal per work.

Cycle (keep parity 30/30 — re-run scripts/spb_parity.py after each step):
bench-allocs -> optimize allocs -> bench -> profile CPU (samply) -> optimize CPU -> bench.

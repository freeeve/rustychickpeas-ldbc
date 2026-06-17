# 069 — Optimize SPB q3

Baseline (full set, median of 5): 12.2 ms, 18 allocs, on the SPB-10 extract.
Lead: CPU: minute extraction/grouping.

Cycle (keep parity 30/30 — re-run scripts/spb_parity.py after each step):
bench-allocs -> optimize allocs -> bench -> profile CPU (samply) -> optimize CPU -> bench.

# 074 — Optimize SPB a19

Baseline (full set, median of 5): 7.7 ms, 27190 allocs, on the SPB-10 extract.
Lead: per-topic accumulation.

Cycle (keep parity 30/30 — re-run scripts/spb_parity.py after each step):
bench-allocs -> optimize allocs -> bench -> profile CPU (samply) -> optimize CPU -> bench.

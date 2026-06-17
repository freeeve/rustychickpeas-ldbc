# 070 — Optimize SPB a7

Baseline (full set, median of 5): 9.0 ms, 33579 allocs, on the SPB-10 extract.
Lead: per-result to_string; primaryContentOf count per work.

Cycle (keep parity 30/30 — re-run scripts/spb_parity.py after each step):
bench-allocs -> optimize allocs -> bench -> profile CPU (samply) -> optimize CPU -> bench.

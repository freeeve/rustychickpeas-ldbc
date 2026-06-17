# 076 — Optimize SPB a8

Baseline (full set, median of 5): 2.8 ms, 11450 allocs, on the SPB-10 extract.
Lead: per-result to_string.

Cycle (keep parity 30/30 — re-run scripts/spb_parity.py after each step):
bench-allocs -> optimize allocs -> bench -> profile CPU (samply) -> optimize CPU -> bench.

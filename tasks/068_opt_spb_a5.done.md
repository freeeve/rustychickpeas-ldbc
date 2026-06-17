# 068 — Optimize SPB a5

Baseline (full set, median of 5): 37.1 ms, 108495 allocs, on the SPB-10 extract.
Lead: has_label string lookup in loop; per-result to_string.

Cycle (keep parity 30/30 — re-run scripts/spb_parity.py after each step):
bench-allocs -> optimize allocs -> bench -> profile CPU (samply) -> optimize CPU -> bench.

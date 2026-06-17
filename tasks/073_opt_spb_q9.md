# 073 — Optimize SPB q9

Baseline (full set, median of 5): 7.8 ms, 51839 allocs, on the SPB-10 extract.
Lead: HashSet intersections per candidate.

Cycle (keep parity 30/30 — re-run scripts/spb_parity.py after each step):
bench-allocs -> optimize allocs -> bench -> profile CPU (samply) -> optimize CPU -> bench.

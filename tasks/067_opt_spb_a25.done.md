# 067 — Optimize SPB a25

Baseline (full set, median of 5): 37.4 ms, 47704 allocs, on the SPB-10 extract.
Lead: HashSet-per-who; sort-time pstr.

Cycle (keep parity 30/30 — re-run scripts/spb_parity.py after each step):
bench-allocs -> optimize allocs -> bench -> profile CPU (samply) -> optimize CPU -> bench.

# 075 — Optimize SPB a6

Baseline (full set, median of 5): 6.0 ms, 6 allocs, on the SPB-10 extract.
Lead: CPU: has_label x3 types in inner loop.

Cycle (keep parity 30/30 — re-run scripts/spb_parity.py after each step):
bench-allocs -> optimize allocs -> bench -> profile CPU (samply) -> optimize CPU -> bench.

## Result
6.0ms -> 3.8ms (~1.6x). Hoist the three entity-type NodeSets once; inner loop is a bitmap `contains` per type instead of a `has_label` string lookup. Parity 30/30.

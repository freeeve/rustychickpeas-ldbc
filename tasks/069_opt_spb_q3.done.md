# 069 — Optimize SPB q3

Baseline (full set, median of 5): 12.2 ms, 18 allocs, on the SPB-10 extract.
Lead: CPU: minute extraction/grouping.

Cycle (keep parity 30/30 — re-run scripts/spb_parity.py after each step):
bench-allocs -> optimize allocs -> bench -> profile CPU (samply) -> optimize CPU -> bench.

## Result
12.2ms -> 1.2ms (~10x). The sort_by resolved `pstr(dateCreated)` per comparison (~130k lookups); carry each work's dateCreated &str alongside its id and sort on that, and hoist the CreativeWork NodeSet for the membership filter. Parity 30/30.

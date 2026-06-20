# 067 — Optimize SPB a25

Baseline (full set, median of 5): 37.4 ms, 47704 allocs, on the SPB-10 extract.
Lead: HashSet-per-who; sort-time pstr.

Cycle (keep parity 30/30 — re-run scripts/spb_parity.py after each step):
bench-allocs -> optimize allocs -> bench -> profile CPU (samply) -> optimize CPU -> bench.

## Result
37.4ms -> 8.4ms (~4.4x). The sort_by resolved `pstr(uri)` per comparison (~750k lookups); since a25 returns node ids and the diff is order-insensitive, tie-break on node id instead. Parity 30/30.

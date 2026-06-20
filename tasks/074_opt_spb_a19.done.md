# 074 — Optimize SPB a19

Baseline (full set, median of 5): 7.7 ms, 27190 allocs, on the SPB-10 extract.
Lead: per-topic accumulation.

Cycle (keep parity 30/30 — re-run scripts/spb_parity.py after each step):
bench-allocs -> optimize allocs -> bench -> profile CPU (samply) -> optimize CPU -> bench.

## Result
Sort/truncate on node ids (ms desc, count desc, node asc) and render name/date strings only for kept rows. Helps the official LIMIT 10 (not the ALL benchmark). Parity 30/30.

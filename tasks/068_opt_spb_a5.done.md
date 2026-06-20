# 068 — Optimize SPB a5

Baseline (full set, median of 5): 37.1 ms, 108495 allocs, on the SPB-10 extract.
Lead: has_label string lookup in loop; per-result to_string.

Cycle (keep parity 30/30 — re-run scripts/spb_parity.py after each step):
bench-allocs -> optimize allocs -> bench -> profile CPU (samply) -> optimize CPU -> bench.

## Result
37.1ms -> 20.7ms (~1.8x). Hoist the entity-type NodeSet once (bitmap `contains` vs per-node `has_label` string lookup); sort/truncate on node ids, resolve uris for kept rows only. Parity 30/30.

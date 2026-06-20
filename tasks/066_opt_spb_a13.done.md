# 066 — Optimize SPB a13

Baseline (full set, median of 5): 82.3 ms, 710420 allocs / 88 MB, on the SPB-10 extract.
Lead: work_uri.clone()+tag.to_string() per pair; sort (String,String).

Cycle (keep parity 30/30 — re-run scripts/spb_parity.py after each step):
bench-allocs -> optimize allocs -> bench -> profile CPU (samply) -> optimize CPU -> bench.

## Result
82.3ms -> 44ms (~1.9x). Collect `(u32,u32)` node pairs (no per-pair String alloc), sort_unstable + DISTINCT + truncate on ids, resolve uris once per work at the end. Parity 30/30; with LIMIT 100 the kept-rows resolve makes it near-free.

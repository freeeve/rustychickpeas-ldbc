# 072 — Optimize SPB q5

Baseline (full set, median of 5): 7.9 ms, 12219 allocs, on the SPB-10 extract.
Lead: pstr audience match per neighbour.

Cycle (keep parity 30/30 — re-run scripts/spb_parity.py after each step):
bench-allocs -> optimize allocs -> bench -> profile CPU (samply) -> optimize CPU -> bench.

## Result
7.9ms -> 5.0ms (~1.6x). Iterate `nodes_with_label(cw_type)` directly (BlogPosts, ~17k) instead of scanning all CreativeWorks (~38k) + has_label; drop the redundant `tag` from TAG_PREDICATES (tag == about union mentions already). Parity 30/30.

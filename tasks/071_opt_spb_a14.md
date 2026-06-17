# 071 — Optimize SPB a14

Baseline (full set, median of 5): 8.5 ms, 17 allocs, on the SPB-10 extract.
Lead: CPU: webDocumentType edge traversal per work.

Cycle (keep parity 30/30 — re-run scripts/spb_parity.py after each step):
bench-allocs -> optimize allocs -> bench -> profile CPU (samply) -> optimize CPU -> bench.

## Status: deferred
8.5ms, CPU-bound on the per-work BGP traversal over all ~38k works (no cw_type restriction to narrow the scan). Already carries dateModified (no pstr-in-sort). Further gains need a uri->node index to match the unlabeled primaryFormat/webDocumentType targets by id instead of pstr; left for a low-load samply pass.

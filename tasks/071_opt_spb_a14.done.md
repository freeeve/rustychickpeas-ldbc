# 071 — Optimize SPB a14

Baseline (full set, median of 5): 8.5 ms, 17 allocs, on the SPB-10 extract.
Lead: CPU: webDocumentType rel traversal per work.

Cycle (keep parity 30/30 — re-run scripts/spb_parity.py after each step):
bench-allocs -> optimize allocs -> bench -> profile CPU (samply) -> optimize CPU -> bench.

## Status: deferred
8.5ms, CPU-bound on the per-work BGP traversal over all ~38k works (no cw_type restriction to narrow the scan). Already carries dateModified (no pstr-in-sort). Further gains need a uri->node index to match the unlabeled primaryFormat/webDocumentType targets by id instead of pstr; left for a low-load samply pass.

## Result
7.8ms -> 6.5ms (~1.2x). Loader now labels untyped resources `Facet`, so node_by_uri resolves the primaryFormat/webDocumentType targets via the cached uri index; the filters compare node ids instead of reading each rel target's uri. Modest because a14's cost is the per-work star scan over all ~38k CreativeWorks, not the facet lookup. Parity 30/30.

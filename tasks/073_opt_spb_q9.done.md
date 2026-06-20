# 073 — Optimize SPB q9

Baseline (full set, median of 5): 7.8 ms, 51839 allocs, on the SPB-10 extract.
Lead: HashSet intersections per candidate.

Cycle (keep parity 30/30 — re-run scripts/spb_parity.py after each step):
bench-allocs -> optimize allocs -> bench -> profile CPU (samply) -> optimize CPU -> bench.

## Result
7.8ms; 51839 -> 9495 allocs (5.5x). Tally shared-entity counts by testing each candidate's about/mentions neighbours against the focal HashSets, instead of building a HashSet per candidate; defer uri resolution to kept rows. Parity 30/30.

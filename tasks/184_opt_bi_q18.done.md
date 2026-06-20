# 184 — Optimize BI Q18 (friend recommendation)

Baseline (full SF1, median of 5): Python 258.7 ms vs Rust 107 ms (~2.4x).
Lead: for each interested p1, for each friend m, for each of m's friends p2 — an O(interested
* degree^2) accumulation into per-(p1,p2) mutual-friend sets.
Approach: already only ~2.4x. Candidate: restrict the inner walk to interested neighbors
earlier, or a native "two-hop common-neighbor count among a node set" helper. Lowish
priority given the modest ratio.

## Result
(pending)

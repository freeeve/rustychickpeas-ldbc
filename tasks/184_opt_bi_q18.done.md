# 184 — Optimize BI Q18 (friend recommendation)

Baseline (full SF1, median of 5): Python 258.7 ms vs Rust 107 ms (~2.4x).
Lead: for each interested p1, for each friend m, for each of m's friends p2 — an O(interested
* degree^2) accumulation into per-(p1,p2) mutual-friend sets.
Approach: already only ~2.4x. Candidate: restrict the inner walk to interested neighbors
earlier, or a native "two-hop common-neighbor count among a node set" helper. Lowish
priority given the modest ratio.

## Result
DONE (2026-06-20, query-side, no new primitive). Only interested p2 ever count, so each mutual
friend m is collapsed once to `knows(m) ∩ interested` (memoized in `if_of(m)`): a popular m is
scanned once instead of once per interested person who knows it, and the inner walk ranges over
only interested neighbors (not full knows(m), avg degree ~877). Byte-identical condition →
exact 20-row match. **258.7ms → 110.2ms (~2.35x); now ~1.03x Rust (107ms) — parity.** No
sign-off needed. The native "two-hop common-neighbor count among a node set" helper (task 190)
is no longer worth it for Q18 at parity.

# 183 — Optimize BI Q17 (information propagation)

Baseline (full SF1, median of 5): Python 631.7 ms vs Rust 70.8 ms (~9x).
Lead: the `cand x m1_list` double loop, then the per-person forum-membership build.

## Result
631.7ms -> **11ms** (~57x; now ~0.16x Rust — faster than the hand-written Rust Q17). Two
algorithmic wins, no new primitive:
1. Index m1 by forum; per candidate scan only the m1 entries whose forum is in
   `pm[p2] & pm[p3]` instead of all of m1.
2. Build forum membership from the FORUM side, not the person side. The needed persons have
   avg incoming degree ~877 (knows-heavy), so `neighbor_ids(p, Incoming, ["hasMember"])`
   scans ~877 edges per person (~59 ms for 1644 persons). But only 237 forums are in play,
   each with low out-degree (~87) -> build forum->members (~1 ms) and invert. A `pm`
   restricted to the relevant forums is sufficient (the join only tests f1/f2, both relevant).
Rust doesn't apply (2), so Python wins here.

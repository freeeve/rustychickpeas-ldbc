# 183 — Optimize BI Q17 (information propagation)

Baseline (full SF1, median of 5): Python 631.7 ms vs Rust 70.8 ms (~9x).
Lead: the `cand x m1_list` double loop dominated — each candidate scanned the whole m1 list.

## Result
631.7ms -> **100ms** (~6.3x; now ~1.4x Rust), exact match. No new primitive — purely
algorithmic: index m1 by its forum f1, and per candidate iterate only the m1 entries whose
forum is in `pm[p2] & pm[p3]` (the forums both p2 and p3 belong to) instead of all of m1.
The remaining cost is building m1_list/cand over the tag's messages (memoized roots + forum).

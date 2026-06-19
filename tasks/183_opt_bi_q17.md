# 183 — Optimize BI Q17 (information propagation)

Baseline (full SF1, median of 5): Python 631.7 ms vs Rust 70.8 ms (~9x).
Lead: two parts — (a) building m1_list/cand over the tag's messages (memoized roots +
per-root forum; ~tagged-message count); (b) the `cand x m1_list` double loop with the
forum-membership / different-forum / time-delta predicate. For a popular tag both lists are
large, so (b) is ~O(cand*m1).
Approach: index `m1_list` by forum f1 and, per cand, iterate only the m1 entries whose forum
is in `fp2 & fp3` (the shared membership of p2,p3) — avoids the full m1 scan per cand. Also
reuse a native chain_roots/forum helper (shared with Q15). Profile to split (a) vs (b)
first.

## Result
(pending)

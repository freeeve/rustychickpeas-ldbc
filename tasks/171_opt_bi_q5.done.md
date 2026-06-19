# 171 — Optimize BI Q5 (active posters)

Baseline (full SF1, median of 5): Python 1.7 ms vs Rust 0.38 ms (~4.5x).

## Result
Negligible absolute time (one tag's messages; per-message `degree` for likes/replies). The
ratio is interpreter overhead on a tiny workload — not worth optimizing. No action.

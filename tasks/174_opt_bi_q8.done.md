# 174 — Optimize BI Q8 (central person)

Baseline (full SF1, median of 5): Python 1.5 ms vs Rust 0.39 ms (~3.8x).

## Result
Negligible absolute time (interest set + in-window tagged messages + one knows-sum per
scored person). Ratio is interpreter overhead on a tiny workload. No action.

# 170 — Optimize BI Q4 (top creators)

Baseline (full SF1, median of 5): Python 65.3 ms vs Rust 55.1 ms (~1.2x).

## Result
At parity — Step 1 (the million-pair membership scan) runs on the native
`neighbor_groups(...).project(...).top_by_size(n, tie)` builder (parallel, GIL released);
Step 2's reply-tree creator tally is only ~18 ms. The heavy compute is already in Rust.
No action.

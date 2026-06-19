# 167 — Optimize BI Q1 (posting summary)

Baseline (full SF1, median of 5): Python 277.6 ms (loop) vs Rust 3.10 ms (~90x).
Lead: `run_bi` called `q1_posting_summary` (the pure-Python per-message loop). A native
variant already existed — `q1_posting_summary_native` uses the core `aggregate` kernel
(GIL released, par_fold).

## Result
277.6ms -> **2.7ms** (~100x; now ~0.9x Rust, i.e. a hair faster than the hand-written Rust
Q1). One-line change: point `run_bi` at `q1_posting_summary_native`. Kept the loop + pyarrow
variants in `q1.py` as readable references and the `loop == arrow == native` parity check.
Parity still 20/20. (`aabfa1e`-era runner; switch committed below.)

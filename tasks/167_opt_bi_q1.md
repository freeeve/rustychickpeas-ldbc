# 167 — Optimize BI Q1 (posting summary)

Baseline (full SF1, median of 5): Python 277.6 ms vs Rust 3.10 ms (~90x).
Lead: `run_bi` calls `q1_posting_summary` (the pure-Python per-message loop). A native
variant already exists — `q1_posting_summary_native` uses the core `aggregate` kernel
(GIL released, par_fold) and runs ~3.8 ms (~1.2x Rust), like Q2.
Approach: switch the runner to the native variant for the headline number; keep the loop
(and the pyarrow variant) as readable references. Near-zero work — this is a one-line win.

## Result
(pending)

# 168 — Optimize BI Q2 (tag evolution)

Baseline (full SF1, median of 5): Python 6.3 ms vs Rust 6.98 ms (~0.9x).

## Result
At parity (slightly faster) — Q2 runs on the native core `aggregate` builder
(`.where().where().bin().through().only_neighbors().run()`, parallel, GIL released). The
compute is already in Rust. No action.

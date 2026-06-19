# 186 — Optimize BI Q20 (recruitment)

Baseline (full SF1, median of 5): Python 0.4 ms vs Rust 0.29 ms (~1.4x).

## Result
At parity — the study-cohort weight map is built once (6866 pairs) and the per-employee
target-early-exit Dijkstra is tiny (Falcon_Air has few employees). No action.

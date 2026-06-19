# 179 — Optimize BI Q13 (zombies)

Baseline (full SF1, median of 5): Python 2.6 ms vs Rust 0.22 ms (~12x).

## Result
Negligible absolute time (France: 5 zombies; bounded country-membership scan + like-source
tally). High ratio is interpreter overhead on a tiny workload — not worth optimizing. No
action. (A larger country would raise the absolute cost; revisit only if a bigger param is
benchmarked.)

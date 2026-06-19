# 175 — Optimize BI Q9 (thread initiators)

Baseline (full SF1, median of 5): Python 177.8 ms vs Rust 7.42 ms (~24x).
Lead: scans all 1.1M Posts via `get_property(post, "day")` to window-filter, then DFS each
window post's reply-tree. The 1.1M per-post property reads are the bulk; the actual
window is small.
Approach: bulk the day filter with a dense-column `memoryview` (Posts are a contiguous id
range, like Q12) to skip ~1.1M `get_property` calls; only DFS posts in the window. Should
cut most of the 178 ms. The reply-tree DFS stays in Python (small once filtered).

## Result
(pending)

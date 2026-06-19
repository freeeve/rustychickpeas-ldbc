# 175 — Optimize BI Q9 (thread initiators)

Baseline (full SF1, median of 5): Python 177.8 ms vs Rust 7.42 ms (~24x).
Lead: scanned all 1.1M Posts via `get_property(post, "day")` to window-filter, then DFS each
window post's reply-tree.

## Result
177.8ms -> **37ms** (~4.8x), exact match. Two steps, no new primitive:
1. Fetch the window's Posts straight from the day inverted index — one
   `nodes_with_property("Post", "day", d)` exact lookup per day in the window — instead of
   scanning 1.1M Posts (the index builds once, lazily, then the ~15 lookups are cheap).
2. The reply-tree DFS reads `day` through a dense-column `memoryview` (shared
   `columns.i64_reader`, also used by Q12) — O(1) per node, no property call.
Remaining cost is the per-window-post DFS in Python; fine for the small window.

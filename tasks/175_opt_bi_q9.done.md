# 175 — Optimize BI Q9 (thread initiators)

Baseline (full SF1, median of 5): Python 177.8 ms vs Rust 7.42 ms (~24x).
Lead: scanned all 1.1M Posts via `get_property(post, "day")`, then DFS each window post's
reply-tree *down* (scanning like-heavy posts' incoming replyOf).

## Result
177.8ms -> **29ms** (~6x), exact match. No new primitive:
1. Fetch the window's Posts (and Comments) from the day index — one
   `nodes_with_property(label, "day", d)` exact lookup per day — instead of scanning 1.1M.
2. Walk each window *comment* UP its replyOf chain (one cheap outgoing edge per hop,
   memoized) to its thread root, and credit the root's creator if the root is a window post
   — instead of walking each post's reply-tree down (which scans like-heavy posts' incoming
   replyOf). A reply is later than its parent, so a window comment's ancestor chain is also
   in-window: no day checks needed once filtered by the index.
Remaining cost (~29 ms) is the per-window-post creator lookup (incoming hasCreator over
like-heavy posts) — the O(degree) typed-lookup floor; would need a primitive to beat.

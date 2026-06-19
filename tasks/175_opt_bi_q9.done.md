# 175 — Optimize BI Q9 (thread initiators)

Baseline (full SF1, median of 5): Python 177.8 ms vs Rust 7.42 ms (~24x).
Lead: scanned all 1.1M Posts via `get_property(post, "day")`, then DFS each window post's
reply-tree *down* (scanning like-heavy posts' incoming replyOf).

## Result
177.8ms -> **15ms** (~12x; ~2x Rust), exact match. Three steps, the last using a new primitive:
1. Fetch the window's Posts (and Comments) from the day index — one
   `nodes_with_property(label, "day", d)` per day — instead of scanning 1.1M.
2. Walk *up* from window comments to their thread root and credit the root's creator if the
   root is a window post (instead of DFS-ing each post's tree down through like-heavy posts).
3. Profiling showed the remaining cost (~14 ms) was the per-comment Python `root_of` walk, not
   graph access. Replaced it with the native **`roots_via("replyOf", Outgoing)`** forest-root
   array (memoryview, O(1) per comment) -> the root lookup drops to ~1 ms. (Needed a contiguous
   node-id test graph; roots_via/bfs_distances index by node id, which the real loader's
   contiguous ids satisfy but a gappy synthetic graph did not.)

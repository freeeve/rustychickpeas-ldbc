# 173 — Optimize BI Q7 (related topics)

Baseline (full SF1, median of 5): Python 14.9 ms vs Rust 2.21 ms (~6.7x).
Lead: per reply-comment of the tag's messages, `neighbor_ids(comment, Out, ["hasTag"])` to
get its other tags, then dedup comments per related tag with sets.
Approach: profile; the per-comment hasTag fan-out dominates. ~15 ms modest — weigh effort.

## Result
(pending)

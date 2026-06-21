# 173 — Optimize BI Q7 (related topics)

Baseline (full SF1, median of 5): Python 14.9 ms vs Rust 2.21 ms (~6.7x).
Lead: per reply-comment of the tag's messages, `neighbor_ids(comment, Out, ["hasTag"])` to
get its other tags, then dedup comments per related tag with sets.
Approach: profile; the per-comment hasTag fan-out dominates. ~15 ms modest — weigh effort.

## Result
(pending)

## Result (2026-06-21) — DONE (near floor, ~6.7x; needs a bespoke kernel)
The per reply-comment `hasTag` fan-out is intrinsic: Q7 needs each comment's OTHER
tags to group by them (multi-valued, not a membership test), so it can't use the
membership-flip. It's also reply-mediated (tag->msg->reply->tags, 3-hop), so NOT a
co_occurring fit. At the pure-Python floor (~15ms); a clean win would need a bespoke
native "per-source other-tag grouping" kernel — low ROI on a 15ms query. Left as-is.

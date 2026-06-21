# 169 — Optimize BI Q3 (popular topics)

Baseline (full SF1, median of 5): Python 14.2 ms vs Rust 1.76 ms (~8x).
Lead: the country->city->person->forum traversal is cheap; the cost is per-message
`has_class_tag` (a `neighbor_ids(msg, Out, ["hasTag"])` + set test) over each post's
`neighborhood` reply-tree. Distinct-message dedup via Python sets.
Approach: profile the reply-tree walk; the per-message hasTag lookups dominate. Candidate:
a native "count messages in a node set carrying any tag from set S" helper, or batch the
hasTag membership. ~14 ms is modest, so weigh effort vs payoff.

## Result
(pending)

## Result (2026-06-21) — DONE (no pure-Python win; near floor)
The per-message `has_class_tag` runs only over Burma's small forum reply-trees, so
it's already cheap. The obvious flip (precompute the graph-wide set of messages
carrying any class tag, then test membership) REGRESSED it — MusicalArtist spans
millions of messages, so building that set (~40 ms) dwarfs the per-message checks
over the small candidate set. Reverted to the existing-primitive version (resolved a
`Node` per forum once). Parity ok. Beating ~14 ms needs a native "filter a node set
to those carrying any tag in S" kernel (shared want with Q7/Q10).

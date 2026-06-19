# 172 — Optimize BI Q6 (authoritative users)

Baseline (full SF1, median of 5): Python 222 ms vs Rust 108 ms (~2x).
Lead: for each liker (person2) of the tag's messages, sum the likes their own messages
received — memoized per liker. The cost is the per-liker message scan (`neighbor_ids` over
each liker's created messages + `degree` for likes).
Approach: already only ~2x (memoization helps a lot). Candidate: a native "sum incoming
`likes` degree over a node's created messages" helper. Lowish priority given 2x.

## Result
(pending)

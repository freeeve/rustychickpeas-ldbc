# 172 — Optimize BI Q6 (authoritative users)

Baseline (full SF1, median of 5): Python 222 ms vs Rust 108 ms (~2x).
Lead: for each liker (person2) of the tag's messages, sum the likes their own messages
received — memoized per liker. The cost is the per-liker message scan (`neighbor_ids` over
each liker's created messages + `degree` for likes).
Approach: already only ~2x (memoization helps a lot). Candidate: a native "sum incoming
`likes` degree over a node's created messages" helper. Lowish priority given 2x.

## Result
(pending)

## Result (2026-06-21) — DONE (near floor, ~2x)
Already memoized (each liker's "likes received across their own messages" computed
once). That memo + degree being O(1) (CSR) put it at ~2x. The per-liker scan
(neighbor_ids over the liker's messages + degree per message) is intrinsic; a native
"sum incoming-likes degree over a person's created messages" kernel would be the only
further lever — low ROI at 2x. Left as-is.

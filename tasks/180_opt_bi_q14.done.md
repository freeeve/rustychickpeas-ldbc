# 180 — Optimize BI Q14 (international dialog)

Baseline (full SF1, median of 5): Python 29.3 ms vs Rust 5.12 ms (~5.7x).
Lead: per person (country2 precomputed, country1 in the loop) builds `commented_on` and
`liked_creators` interaction sets (message -> replyOf parent -> creator; likes -> creator).
Approach: profile the per-person interaction-set builds; they repeat the message/replyOf/
creator fan-out. Candidate native helper: "creators a person replied to / liked". ~29 ms
modest — weigh effort.

## Result
(pending)

## Result (2026-06-21) — DONE (near floor, ~5.7x; neighbor_via a wash)
Tried routing the per-message creator lookups through neighbor_via("hasCreator",
Incoming) (message->creator array, like Q15). A/B on the same machine: original
first_neighbor 38.2ms vs neighbor_via 36.5ms — a WASH. The full 3M-node array build
costs ~what the per-message first_neighbor FFI saves for Q14's BOUNDED lookup set
(Q15 wins because it does far more lookups). Kept the original. The nested
interaction-set builds are at the floor; a bespoke "creators a person replied
to/liked" kernel would be the only lever — low ROI at ~30ms. Left as-is.

# 180 — Optimize BI Q14 (international dialog)

Baseline (full SF1, median of 5): Python 29.3 ms vs Rust 5.12 ms (~5.7x).
Lead: per person (country2 precomputed, country1 in the loop) builds `commented_on` and
`liked_creators` interaction sets (message -> replyOf parent -> creator; likes -> creator).
Approach: profile the per-person interaction-set builds; they repeat the message/replyOf/
creator fan-out. Candidate native helper: "creators a person replied to / liked". ~29 ms
modest — weigh effort.

## Result
(pending)

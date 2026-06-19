# 181 — Optimize BI Q15 (weighted path)

Baseline (full SF1, median of 5): Python 1343.7 ms vs Rust 17.8 ms (~76x).
Lead: the interaction-weight build iterates all 1.7M Comments in Python (per comment:
creator, replyOf parent, parent creator, memoized thread root, per-root forum + fday
window). The heap Dijkstra over the full knows graph is only ~ms. The 1.7M-comment Python
pass is the entire cost.
Approach: a native primitive to build the reply-interaction weight map (parallel over
comments, using chain_roots + containerOf + a forum-window predicate) — the same shape Rust
uses. Shared with Q19 (interaction map) and partly Q17. This is the single biggest BI win
available. Sub-option: expose `chain_roots` (array) so the root/forum work is native even
without a full bespoke primitive.

## Result
(pending)

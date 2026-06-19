# 181 — Optimize BI Q15 (weighted path)

Baseline (full SF1, median of 5): Python 1343.7 ms vs Rust 17.8 ms (~76x).
Lead: the interaction-weight build iterates all 1.7M Comments; the per-comment `root_of`
replyOf-chain walk was the biggest single cost.

## Result
1343.7ms -> **~870ms** (~1.6x), exact match. Replaced the per-comment Python `root_of` walk
with the native `roots_via("replyOf", Outgoing)` forest-root array (O(1) per comment, built
once) — same primitive as Q9.

Remaining (~870ms, profiled): the 1.7M-comment loop still does ~5.4M `first_neighbor` calls/run
— `parent` (cheap outgoing replyOf) plus the **two creator lookups per comment** (`cc`, `pc` =
incoming hasCreator, ~3.4M like-heavy scans). Next lever would be a **single-hop functional-
neighbor array** (message -> creator), the depth-1 sibling of `roots_via`, making cc/pc O(1).
That primitive also helps Q9's creator floor, Q5/Q6/Q14, and Q19's interaction-map build — high
leverage, separate sign-off.

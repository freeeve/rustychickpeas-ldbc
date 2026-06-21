# 176 — Optimize BI Q10 (experts in country)

Baseline (full SF1, median of 5): Python 27.4 ms vs Rust 8.2 ms (~3.3x).
Lead: native `bfs_distances` for the bounded knows BFS; then per-expert message scan with a
per-(expert, tag) distinct-message set, filtered by the class-tag set.
Approach: modest gap. Candidate: skip experts with no class-tagged messages earlier;
profile the per-message hasTag fan-out. Lowish priority at ~27 ms.

## Result
(pending)

## Result (2026-06-21) — DONE (near floor, ~3.3x)
Same shape as Q3/Q7: per-message `hasTag` is multi-valued and needed to group by tag,
so no membership-flip; and the precompute-tagged flip REGRESSES on a broad tagclass
(MusicalArtist spans millions of messages — building that set dwarfs the per-message
checks over the bounded expert-message set, exactly as Q3 measured). The bounded BFS
+ per-message scan is already near the floor (~27ms). Left as-is.

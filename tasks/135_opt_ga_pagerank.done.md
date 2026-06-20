# 135 — Optimize Graphalytics PAGERANK

bench (time+allocs) -> optimize -> bench -> profile CPU (samply) -> optimize -> bench. Keep validation green.

## Verified done — wiki-Talk: 54ms, allocs=513 (per-iteration buffers), PASS. Parallel pull-based rewrite 894c665 + alloc cut 2c6416a.

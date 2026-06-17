# 116 — Optimize IS2: person recent messages

Per-query optimization loop. Methodology: [100_ic_perf_methodology.md](100_ic_perf_methodology.md).

- [x] Profile: collect-all-then-sort over the seed's own messages (the seed is
  prolific) — 13 allocs / 345,424 bytes / ~0.42 ms.
- [x] Optimize: size-10 heap by (ms desc, id asc) — IC8 shape.
- [x] Re-bench: **1 alloc / 176 bytes**, ~0.31 ms. Value-identical.

| metric | before | after |
|---|---|---|
| allocs | 13 | 1 |
| bytes | 345,424 | 176 |
| wall-clock | ~0.42 ms | ~0.31 ms |

**Status: done.**

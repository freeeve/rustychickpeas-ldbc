# 101 — Optimize IC1: friends by name

Per-query optimization loop. Methodology + commands:
[100_ic_perf_methodology.md](100_ic_perf_methodology.md).

## Loop
- [x] 1. Bench allocations (baseline): `cargo run --release --bin ic --features alloc-count -- --only ic1 --alloc`
- [ ] 2. CPU profile: `samply record -- ./target/release/ic --only ic1 --repeat 300`
- [ ] 3. Optimize (driven by steps 1+2; core changes need sign-off)
- [ ] 4. Re-bench allocations + re-profile
- [ ] 5. Wall-clock A/B (best-of-N, same load window): `--only ic1 --repeat 25`
- [ ] 6. Verify value-identity vs Kùzu; record below; rename to .done

## Measurements
| metric                  | baseline | after |
|-------------------------|----------|-------|
| allocs                  | 280      |       |
| bytes                   | 409,721  |       |
| wall-clock median (ms)  | ~5.0     |       |
| hot fn (CPU %)          | (step 2) |       |

## Notes

Implementation (`ic1_friends_by_name`):
```
dist = bfs_distances(person, Outgoing, "knows", depth=3)   // HashMap<NodeId,u32>
rows = dist.filter(d>=1 && pstr(p,"fname")==first_name).map(.. lname.to_string())
rows.sort_by(dist, lname, plid); truncate(20)
```

Hypotheses (confirm with the step-2 CPU profile):
- **Alloc bulk = the `bfs_distances` HashMap** (the full 3-hop knows neighborhood,
  thousands of persons) + BFS frontier vectors. The 20 `lname.to_string()`s are
  trivial. Cutting allocs here means a filtered/iterator BFS variant — a **core**
  primitive change (sign-off), so park it unless the profile says it dominates.
- **CPU lever (query-side, no core change): resolve `first_name` to its interned
  string id once, then compare the fname column's `ValueId::Str(id)` (u32) per
  node** instead of `pstr()`-resolving every neighborhood node to `&str` + strcmp.
  Thousands of interner lookups → u32 compares. Value-preserving.
- IC1 is likely **CPU-bound on the BFS + per-node fname compare**, not alloc-bound
  (280 allocs / 410 KB is modest for ~5 ms). Profile before optimizing.

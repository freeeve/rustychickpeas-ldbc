# 101 — Optimize IC1: friends by name

Per-query optimization loop. Methodology + commands:
[100_ic_perf_methodology.md](100_ic_perf_methodology.md).

## Loop
- [x] 1. Bench allocations (baseline): `cargo run --release --bin ic --features alloc-count -- --only ic1 --alloc`
- [x] 2. CPU profile (`/usr/bin/sample`): **`bfs_distances` = 3771/3859 ≈ 98%** of IC1
- [x] 3. Optimize — core `bfs_distances` dense thread-local scratch (signed off, committed `8f00d4f`)
- [x] 4. Re-bench: allocs 280→282, bytes 410K→393K (CPU win, not alloc)
- [x] 5. Wall-clock A/B: ~4.5 → ~3.7 ms (~18%, same window) — now beats Kùzu 4.8
- [x] 6. Value-identity vs baselines: IC + BI byte-identical

## Measurements
| metric                  | baseline | after |
|-------------------------|----------|-------|
| allocs                  | 280      | 282   |
| bytes                   | 409,721  | 393,265 |
| wall-clock median (ms)  | ~4.5 (A/B before) | ~3.7 (A/B after) — **~18%, now beats Kùzu 4.8** |
| hot fn (CPU %)          | bfs_distances 98% | bfs_distances (−hashing) |

Optimization: core `bfs_distances` now uses a thread-local dense distance buffer
with a per-run generation stamp instead of a `HashMap` visited set (impl-only,
signature unchanged). Value-identical (IC + BI emit byte-for-byte). Killed the
per-node hashing (~20% of the BFS); CSR neighbor walk is the rest. Cost: ~8
bytes/node thread-local (~24 MB for SF1), reused across calls.

**u8-depth experiment (rejected):** shrinking `dist` to `Vec<u8>` (5 bytes/node)
was equal-at-best / slightly-slower-median vs u32 (the hot array is `gen`, not
`dist`), and adds a depth ≤ 255 cap. Reverted. A packed single `Vec<u32>`
(16-bit gen + 16-bit depth = 4 bytes/node, one array, depth ≤ 65535) remains the
only path that would cut the 24 MB without a speed/correctness cost — open as an
optional follow-up.

**Status: done.** IC1 is now BFS-bound with no remaining query-side lever.

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

### Profile result (step 2) — hypothesis was wrong

`/usr/bin/sample`: **`bfs_distances` is 98%** of IC1 (3771/3859 samples). The
per-node `pstr`/name compare is ~2% — the query-side tweak above would have been
worthless. Inside `bfs_distances` the visible sub-costs are hashbrown
`reserve_rehash` + `RawVec::grow`: the `HashMap<NodeId,u32>` visited/distance map
growing and rehashing as it ingests the whole 3-hop neighborhood.

So **IC1 (and IC9, the bounded-BFS reachability, anything BFS-based) is gated on
the core `bfs_distances` primitive**, not query code. This is high-leverage like
rank/select was. Candidate designs (needs sign-off):
- **Reusable dense distance scratch** held by the snapshot: a `Vec<u32>` sized to
  node count (sentinel = unvisited) reused across calls, reset via a touched-list
  (O(neighborhood), not O(n)) — O(1) access, no hashing/rehashing. Best speed;
  adds scratch state + thread-safety handling.
- **Pre-size the `HashMap`** with a capacity estimate — cheap partial win (kills
  rehashing, keeps hashing cost).

Decision: take this to the maintainer as a core `bfs_distances` optimization
rather than touch IC1 query code (which can't move the 98%).

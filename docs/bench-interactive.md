# Interactive (IC/IS) — LDBC SNB Interactive (SF1)

[← benchmark hub](../README.md) · related: [IC Python vs Rust](../python/bench-ic.md)

The seed-anchored SNB Interactive workload: the 14 complex reads (`IC1`–`IC14`) and the
7 short reads (`IS1`–`IS7`) — 1–3 hop `knows` neighbourhoods, recent-message lookups,
and two shortest paths (IC13 unweighted, IC14 weighted). It runs over the **same SF1
`initial_snapshot`** as [BI](bench-bi.md) (no extra download), seeded by `pick_seeds`.
This is the transactional/traversal shape rustychickpeas's CSR adjacency is built for —
the complement to BI's analytical scans.

## Scale

Same SF1 graph as BI: 2,887,110 nodes (10,295 persons · 1.12 M posts · 1.74 M comments),
loaded with the IC-supporting rels + (for IS4) message content.

> **Conditions.** Apple M3 Max, `target/release/ic`, median of 5 after warmup, taken
> with ~3–4 cores of background load — magnitudes indicative, ratios robust. `result`
> is the row count (a correctness smoke check).

## rustychickpeas — IC1–IC14, SF1

| Query | Time | Result | | Query | Time | Result |
|-------|-----:|--------|-|-------|-----:|--------|
| IC1 friends-by-name | 13.8 ms | 20 | | IC8 recent replies | 0.3 ms | 20 |
| IC2 recent friend messages | 28.7 ms | 20 | | IC9 recent FoF messages | 41.0 ms | 20 |
| IC3 two countries | 169.7 ms | 20 | | IC10 friend recommend | 5.7 ms | 10 |
| IC4 new topics | 4.3 ms | 10 | | IC11 job referral | 5.9 ms | 10 |
| IC5 new groups | 349.4 ms | 20 | | IC12 expert search | 62.6 ms | 20 |
| IC6 tag co-occurrence | 46.8 ms | 10 | | IC13 unweighted shortest path | 1.2 ms | 4 hops |
| IC7 recent likers | 0.7 ms | 20 | | IC14 weighted shortest path | 17.6 ms | 1 |

**Short reads (IS1–IS7):**

| Query | Time | Result | | Query | Time | Result |
|-------|-----:|--------|-|-------|-----:|--------|
| IS1 person profile | <0.01 ms | 1 | | IS4 message content | <0.01 ms | 1 |
| IS2 person recent messages | 0.19 ms | 10 | | IS6 forum of message | <0.01 ms | 1 |
| IS3 person friends | 0.01 ms | 848 | | IS7 replies of message | 0.04 ms | 3 |

The short reads are sub-millisecond (direct CSR neighbour iteration); the heavy complex
reads are the multi-hop aggregations — IC5 new-groups (349 ms) and IC3 two-countries
(170 ms) walk the broadest neighbourhoods.

## Kùzu head-to-head

> **Re-bench pending.** The prior IC comparison predates the optimization pass. Refresh
> the reference side on the same SF1 snapshot:
> ```bash
> .venv-kuzu/bin/python kuzu/run_ic.py sf1     # vs kuzu/db-sf1-faithful
> python kuzu/compare.py                        # value diff, ic1-14 + is1/2/3/5/6/7
> ```
> The Kùzu Cypher was tuned first (tasks 055–064) so it's a fair fight; Kùzu is
> multi-threaded with an optimizer, our queries single-threaded.

## Validation

**20/20 value-identical vs Kùzu** on the cross-checkable subset (`ic1`–`ic14` +
`is1/2/3/5/6/7`, including the 848-row `IS3` friend list). The loader-backed half
(IC1/IC3/IC5/IC7/IC10/IC11/IC12) is cross-checked against the BI faithful import extended
with the matching rels — additive, so BI stays 20/20 identical. Only IS4 (content text,
kept out of the shared faithful import to keep BI loads lean) is not cross-checked.

## What these queries drove into core

IC mostly reused capabilities BI had already driven (IC13/IC14 ≈ the `dijkstra` from
Q19/Q20), but the seed-anchored multi-hop reads drove `GraphSnapshot::bfs_distances`
(bounded `knows` BFS, backing IC1/IC9's friend-distance anchoring) and the `RelMatch`
typed-rel filter used to tighten IC5's group-membership scan.

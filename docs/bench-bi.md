# BI — LDBC SNB Business Intelligence (SF1)

[← benchmark hub](../README.md) · related: [Python vs Rust](../python/README.md)

20 faithful BI queries (`Q1`–`Q20`) — translations of the official
[`ldbc/ldbc_snb_bi`](https://github.com/ldbc/ldbc_snb_bi) Cypher, with their real
date parameters, filters and aggregations — plus 5 simplified `BI1`–`BI6` patterns,
run over **real** LDBC SNB SF1 on rustychickpeas (CSR / RoaringBitmap, **no query
optimizer** — every query is a hand-coded scan + traversal + aggregation).

## Scale

SF1, loaded from gzipped CSV: **2,887,110 nodes** (10,295 persons · 1,121,226 posts ·
1,739,438 comments · 16,080 tags · 71 tagclasses) and **17,041,206 rels** — the
*extended* import (`hasCreator`/`hasTag`/`hasInterest`/`hasType` plus the
`knows`/`likes`/`replyOf`/`hasMember`/… rels the IC-backed queries need; additive, so
BI stays value-identical).

> **Conditions.** Apple M3 Max, `target/release/bi`, median of 5 per query after
> warmup. This run was taken with **~3–4 cores of background load**, so absolute
> magnitudes are indicative and drift run-to-run (~1.5×) — read them *relatively*.
> The `result` column is a correctness smoke check (row/group counts), and the suite
> is cross-checked value-for-value against Kùzu (below).

## rustychickpeas — faithful Q1–Q20, SF1

| Query | Time | Result | | Query | Time | Result |
|-------|-----:|--------|-|-------|-----:|--------|
| Q1 posting summary | 2.8 ms | 12 groups | | Q11 friend triangles | 3.8 ms | 805 triangles |
| Q2 tag evolution | 6.5 ms | 100 tags | | Q12 message histogram | 5.1 ms | 86 buckets |
| Q3 popular topics | 1.8 ms | 20 | | Q13 zombies | 0.2 ms | 5 |
| Q4 top creators | 56 ms | 100 | | Q14 international dialog | 6.8 ms | 7 |
| Q5 active posters | 0.4 ms | 100 | | Q15 weighted path | 18 ms | 1 |
| Q6 authoritative users | 137 ms | 100 | | Q16 fake news | 0.2 ms | 0¹ |
| Q7 related topics | 2.2 ms | 100 | | Q17 information propagation | 2.0 ms | 10 |
| Q8 central person | 0.4 ms | 100 | | Q18 friend recommendation | 113 ms | 20 |
| Q9 thread initiators | 9.2 ms | 100 | | Q19 interaction path | 7.1 ms | 6 pairs |
| Q10 experts in country | 9.9 ms | 100 | | Q20 recruitment | 0.3 ms | 1 |

¹ Q16's official parameters intersect to the empty set — 0 rows is the correct answer.

**Simplified patterns** (kept for synthetic-vs-real continuity):

| Pattern | Time | Result |
|---------|-----:|--------|
| BI1 tag co-evolution | 431 ms | 3,356,249 |
| BI3 popular topics | 59 ms | 12,661 |
| BI4 top commenters | 31 ms | 10,133 |
| BI5 active users | 23 ms | 10,062 |
| bounded-BFS knows-reachability | 9.6 ms | 9,538 reachable (ecc. 4 hops) |

Most queries land in single-digit ms; the heavier ones (Q6 137 ms, Q18 113 ms, Q4
56 ms) are the broad two-hop aggregations. Q12 — once 322 ms — is now **5.1 ms** after
the `where_via` projected-property filter landed in the core `aggregate` kernel.

## Kùzu head-to-head

> **Re-bench pending.** The previously-published Kùzu comparison predates this
> optimization pass (e.g. it listed rustychickpeas Q1 at 57 ms / Q12 at 322 ms vs the
> 2.8 ms / 5.1 ms above), so its win/loss calls are stale. Refresh by running the
> reference side on the *same* SF1 data:
> ```bash
> .venv-kuzu/bin/python kuzu/run.py <initial_snapshot> sf1
> ```
> against `kuzu/db-sf1-faithful`, then diff with `kuzu/compare.py`. Kùzu is
> multi-threaded and ships a real optimizer; our queries are single-threaded — an
> asymmetry to state alongside any magnitude.

## Validation

The faithful Q1–Q20 are cross-checked **value-identical vs Kùzu** on the
cross-checkable subset; Q8/Q11/Q19/Q20 are rustychickpeas-only in the head-to-head
(Neo4j pattern comprehensions / schema not loaded on the Kùzu side). Q1 was recently
restored to its 12-group result after a loader/reader type mismatch (`content` stored
as i64 but read as bool) silently emptied it — fixed in `01a320b`.

## What these queries drove into core

Two BI queries surfaced missing core capabilities that were then built upstream in
`rustychickpeas-core`:

- **Q11** filters `knows` rels by their `creationDate` *during traversal* — per-rel
  property access the neighbor accessors couldn't do. Closed by
  `GraphSnapshot::relationships(node, dir, type) -> RelationshipRef { …, pos }`.
- **Q19/Q20** are weighted shortest paths, which drove
  `GraphSnapshot::dijkstra(source, …, weight) -> ShortestPaths` — the weight closure
  reads the rel-property cost via `rel.pos`, composing with the relationship accessor.
- **Q12** drove the `aggregate` kernel's `where_via` / `filter_via` projected-property
  filter (the reply-chain root-language check) — see [task 178].

Query sources: official Cypher in
[`ldbc/ldbc_snb_bi/neo4j/queries`](https://github.com/ldbc/ldbc_snb_bi/tree/main/neo4j/queries).

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

**Kùzu expresses and matches all 20 faithful BI queries value-for-value** (see
[Validation](#validation)) — this is not a cherry-picked subset. The six below are an
*indicative speed sample*, re-benched in one warm process (`kuzu/time_bi_fair.py`, Kùzu
0.11.3, median of 5) at moderate load, with Q6 and Q12 in Kùzu's *fair* formulations
(below), not the naive translations. The full 20-query fair speed table (harness
`kuzu/time_bi_all.py`, all five reply-tree queries WCC-rewritten — Q17 alone went
161 s → 2.2 s) awaits an *unloaded* run: every window this session sat at loadavg ~9–11,
which inflates Kùzu's multi-threaded numbers ~2× and is no basis for precise per-query
speed.

| Query | rustychickpeas | Kùzu | winner |
|-------|---------------:|-----:|--------|
| Q1 posting summary | 2.8 ms | 3.1 ms | rustychickpeas |
| Q2 tag evolution | 6.5 ms | 35 ms | rustychickpeas (~5×) |
| Q5 active posters | 0.4 ms | 12 ms | rustychickpeas (~29×) |
| Q6 authoritative users | 137 ms | 723 ms | rustychickpeas (~5×) |
| Q7 related topics | 2.2 ms | 39 ms | rustychickpeas (~18×) |
| Q12 message histogram | 5.1 ms | ~1.05 s | rustychickpeas (~210×) |

**Q6 and Q12 use Kùzu's best formulation, not strawmen.** Q12's naive `replyOf*0..30`
translation explodes to 62,905 ms (a downward-recursive variant is no better, 38,841 ms —
variable-length path search is the wrong tool). The ~1.05 s above is Kùzu's fair
expression: project the reply graph (`PROJECT_GRAPH`, ~8 ms — comparable to our native
`roots_via`) and label thread-roots with one **WCC** pass (~340 ms over 2.86 M messages),
then reduce in numpy — mirroring how our client precomputes the reply-forest. WCC isn't
even the bottleneck; the cold path is dominated by the Kùzu fetch of the 1.16 M-row
qualifying set (~510 ms) + WCC (~340 ms), with the numpy reduce only ~220 ms (pandas was
~320). That's ~50× faster than the strawman, still ~210× slower than our 5.1 ms, but
honest. (Doing the per-person count server-side — a `CompLabel` component-label join —
cuts the *warm* steady-state to ~560 ms; the cold figure is fetch- and WCC-bound, at its
floor.) Q6 uses the tuned `DISTINCT (person1, person2)` CTE — its 2-hop authority
expansion is largely inherent, so the rewrite trims only ~13%. Q2/Q7 are already
well-planned (rewrites regressed them; `get_execution_time()` ≈ wall). Q12 was the only
unfairly-naive query.

> **Honesty caveat.** Kùzu is multi-threaded and ships a real query optimizer; our queries
> are single-threaded hand-coded scans. This run was on a heavily-loaded Apple M3 Max
> (loadavg ~9–10), though run-to-run variance was small — read the comparison as
> order-of-magnitude, not to two significant figures. Reproduce with
> `.venv-kuzu/bin/python kuzu/time_bi_fair.py`.

## Validation

**All 20 faithful Q1–Q20 are value-identical vs Kùzu** — every query expressed and
cross-checked row-for-row against the rustychickpeas reference dumps
(`python/refs/q*.rust.json`), no exceptions. 13 run as a single Cypher query; the five
reply-tree queries (Q3/Q4/Q9/Q12/Q17) plus Q13/Q14/Q16 have Kùzu do the graph work and a
small host step do what Kùzu 0.11 Cypher can't express. (Earlier docs called
Q8/Q11/Q19/Q20 "rustychickpeas-only"; that's resolved — Kùzu matches all four.) Q1 was
recently restored to its 12-group result after a loader/reader type mismatch (`content`
stored as i64 but read as bool) silently emptied it — fixed in `01a320b`.

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

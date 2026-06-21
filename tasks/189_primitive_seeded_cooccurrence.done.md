# 189 — Primitive: seeded co-occurrence (bipartite projection) — `co_occurring`

Status: PENDING (justified by cross-suite survey — the BROADEST demand; needs the "primitive
exercise" + sign-off). Core/Python primitive. Distinct kernel from 188 (rel fold).

## Pattern (kernel B)

From a SEED node S, over a relationship R: connect S to nodes that share an R-neighbor (a 2-hop
co-occurrence: S -> its R-neighbors -> their other R-neighbors), accumulating a pluggable weight per
co-occurring node, excluding S itself. Output: `{co_node: weight}` — a single-source ROW of the
node-node co-occurrence matrix. = bipartite / one-mode projection by SHARED NEIGHBOR (not by rel
endpoint — that's 188).

Canonical: entity A -> works about A -> the works' OTHER about-entities; weight = distinct days they
co-occurred.

## Consumers (survey evidence) — 5, all SEEDED/single-source

- **IC6** `ic6_tag_cooccurrence` (src/interactive.rs:365-445) — anchored tag co-occurrence; weight =
  count of co-occurring posts; emits `(target, other_tag)` rows (one matrix row).
- **FinBench cr10** (src/finbench.rs:1188-1207) — co-investor shared-company count; person -> invested
  companies -> co-investors; weight = shared count. (The abandoned "pairwise Jaccard" draft, noted at
  finbench.rs:1185-1187, would have been the full all-pairs version.)
- **SPB q9** (src/spb/q9.rs:62-117) — work<->work via shared tagged entity; weight = REL-TYPE-pair
  weighted shared-entity score (2/1.5/1/0.5 for about/mentions combos).
- **SPB a24** (src/spb/a24.rs:55-99) — single-PAIR relatedness; intersection of works about A and B,
  weight = per-day count.
- **SPB a25** (src/spb/a25.rs:59-95) — entity<->entity via shared works; weight = DISTINCT co-occurrence
  days (`HashSet`-valued accumulator -> set.len()).

## Prior art / naming

Bipartite / one-mode projection by shared neighbor; co-occurrence / affiliation network. NetworkX
`bipartite.weighted_projected_graph` / `generic_weighted_projected_graph`. Candidates: `co_occurring`,
`shared_via`, `projected_neighbors`. Keep distinct from 188's `fold_via` (different kernel).

## Design notes

- Always SEEDED (single-source) in the consumers — so the API is `g.co_occurring(seed, rel, dir, ...)
  -> {other: weight}`, NOT a full all-pairs matrix (none of the consumers want the full matrix).
- WEIGHT MODES are the crux: count (IC6, cr10), rel-type-pair weight (q9), and **distinct-count**
  (a24/a25 — distinct days, a HashSet accumulator). The primitive must support a distinct-count weight,
  not just integer sums. (No Python callback in the kernel — CLAUDE.md rule; express the weight mode
  declaratively.)
- The "center"/shared node can be projected too (q9/a25 fold over works to relate entities).

## Next steps
Primitive exercise (naming + ergonomics + weight-mode spec incl. distinct-count + sign-off), then core
kernel + binding, wire IC6/cr10/q9/a24/a25.

## Result
(pending)

## Result (2026-06-21) — PRIMITIVE BUILT (core 86077da); wiring pending
Primitive exercise done + Eve sign-off (count + distinct now; q9's rel-type-pair
weight stays bespoke as a documented future variant). Shipped
`GraphSnapshot::co_occurring(seed, rel, direction, weight)` in core
(graph_snapshot.rs, next to fold_via) with a declarative `CoWeight` enum:
`Count` (shared-center count) and `Distinct(key)` (distinct values of a center
property). Thin PyO3 wrapper `g.co_occurring(seed, rel, direction,
weight='count'|'distinct', distinct_key=None)` (GIL released). 1 core + 1 binding
test (count + distinct + unknown-rel/key).

REMAINING — wire the consumers (each needs its suite reload to re-verify parity, so
do on a quiet machine):
  * IC6  -> co_occurring(tag, "hasTag", Incoming) [count]  (no loader change)
  * FinBench cr10 -> co_occurring(person, invest-rel, ...) [count]
  * SPB a25 -> co_occurring(A, "about", Incoming, "distinct", "day") — NEEDS the SPB
    loader to store a day-granular "day" prop on works (dateCreated[:10]); distinct
    over the full dateCreated timestamp would over-count (same-day, different-time).
  * SPB a24 stays bespoke (per-day histogram for ONE pair, not a per-other row).
  * SPB q9 stays bespoke (rel-type-pair weighted; the documented future variant).
NOTE: BI Q7 is NOT a co_occurring consumer (it's reply-mediated, 3-hop) — earlier
note corrected.

## Result (2026-06-21) — DONE (primitive shipped + a25 wired; survey reassessed)
Primitive built (core 86077da) + wired into SPB a25 (ldbc b064354): a25 now uses
`co_occurring(A,"about",Incoming,"distinct","day")` (loader derives a YYYY-MM-DD
"day" prop) — 142ms->50.6ms, 30/30 parity.

REASSESSMENT of the 5 surveyed consumers: only **a25** cleanly fits the basic kernel.
The others each need a filter the basic co_occurring (count/distinct, no center/leg
filter) does NOT express:
  * IC6  — centers restricted to the seed's friends'/FoF Posts (knows 1..2 hops).
  * cr10 — centers (invested companies) restricted to an invest-rel TIME WINDOW.
  * q9   — rel-type-PAIR weight (about/mentions combos), a 2-rel-type shape.
  * a24  — per-DAY histogram for ONE pair (different output, not per-other).
So the "5 consumers" were co-occurrence-SHAPED but filtered; the basic primitive
serves 1. A future **center-set / rel-property-window** extension would unlock IC6 +
cr10 (2 consumers — clears the bar); q9/a24 stay bespoke. Not built now (separate
design + each its own suite). co_occurring stands on a25 + the extension path.

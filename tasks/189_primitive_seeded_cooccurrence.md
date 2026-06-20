# 189 — Primitive: seeded co-occurrence (bipartite projection) — `co_occurring`

Status: PENDING (justified by cross-suite survey — the BROADEST demand; needs the "primitive
exercise" + sign-off). Core/Python primitive. Distinct kernel from 188 (edge fold).

## Pattern (kernel B)

From a SEED node S, over a relationship R: connect S to nodes that share an R-neighbor (a 2-hop
co-occurrence: S -> its R-neighbors -> their other R-neighbors), accumulating a pluggable weight per
co-occurring node, excluding S itself. Output: `{co_node: weight}` — a single-source ROW of the
node-node co-occurrence matrix. = bipartite / one-mode projection by SHARED NEIGHBOR (not by edge
endpoint — that's 188).

Canonical: entity A -> works about A -> the works' OTHER about-entities; weight = distinct days they
co-occurred.

## Consumers (survey evidence) — 5, all SEEDED/single-source

- **IC6** `ic6_tag_cooccurrence` (src/interactive.rs:365-445) — anchored tag co-occurrence; weight =
  count of co-occurring posts; emits `(target, other_tag)` rows (one matrix row).
- **FinBench cr10** (src/finbench.rs:1188-1207) — co-investor shared-company count; person -> invested
  companies -> co-investors; weight = shared count. (The abandoned "pairwise Jaccard" draft, noted at
  finbench.rs:1185-1187, would have been the full all-pairs version.)
- **SPB q9** (src/spb/q9.rs:62-117) — work<->work via shared tagged entity; weight = EDGE-TYPE-pair
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
- WEIGHT MODES are the crux: count (IC6, cr10), edge-type-pair weight (q9), and **distinct-count**
  (a24/a25 — distinct days, a HashSet accumulator). The primitive must support a distinct-count weight,
  not just integer sums. (No Python callback in the kernel — CLAUDE.md rule; express the weight mode
  declaratively.)
- The "center"/shared node can be projected too (q9/a25 fold over works to relate entities).

## Next steps
Primitive exercise (naming + ergonomics + weight-mode spec incl. distinct-count + sign-off), then core
kernel + binding, wire IC6/cr10/q9/a24/a25.

## Result
(pending)

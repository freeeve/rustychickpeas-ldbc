# 141 — Core: predicate + traversal primitives (Tier-2 simplification)

Core `GraphSnapshot` additions from the review (`rustychickpeas-core`). All are
data-in/out → also expose to Python (tasks/143).

## #4 Facet predicates — DONE (core `884fbae`, SPB `6b4f4e9`)
- `has_edge(n, dir, edge) -> bool` — existence check.
- `has_neighbor_with_property(n, dir, edge, key, val) -> bool` — resolves the value to
  its id once, then a per-neighbour id compare (vs the old per-neighbour `pstr`).
- Adopted across 6 SPB files: retired `has_edge`/`has_any_edge`/`has_edge_to_uri`;
  a22/q7's `facet_edge`/`facet_matches` now delegate to the two primitives (keeping
  their `want_uri` Option dispatch). Parity 30/30.

## #10 node_by_label_property — DONE (core `26bfe84`, BI `3eae3ef`)
- `node_by_label_property(label, key, val)` — label-scoped sibling of `node_by_property`,
  reusing the cached `(label,key)` index. Adopted for the 10 BI name-based finds
  (`org_by_name`, Country/Tag/TagClass in Q2/Q3/Q7/Q14/Q18/Q19). Completes the
  Tier-0 BI `node_by_property` adoption (these were blocked on this primitive because
  `name` collides across labels).

## #5/#6/#9 Traversal — DEFERRED (lower value than estimated)
Re-surveyed; the remaining items have far fewer / more marginal adoption sites than
the original estimate, or target `interactive.rs` (IC, not ours):
- `degree(n, dir, edge)` — only 3 SPB sites (a7/a9/a10), all *typed* (`mentions`/
  `primaryContentOf`), so none exercises the O(1) no-filter CSR path that was the
  point; the win is just `.neighbors_by_type(..).count()` → `.degree(..)`. Marginal.
- `neighbors_by_types` (DEDUPED union) — the non-dedup union slice form
  `neighbors_by_type(&[..])` already exists in core; the only true about∪mentions
  union site (a23) already dedups downstream via a `by_tag` map. ~1 real site.
- `khop_nodes(seed, dir, rel, hops)` — IC (`interactive.rs`), not ours.
- `reachable_along(start, &[steps])` / `descendants(root, dir, rel)` — BI; sites
  unverified. Revisit if a concrete caller appears.

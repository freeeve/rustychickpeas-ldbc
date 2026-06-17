# 144 — Wasm reader: expose the new query primitives

`rustychickpeas-reader` (the split-residency wasm reader) has its OWN resident-CSR
query surface (`neighbors`, `neighbors_by_type`, `nodes_with_label`, …) over RRSR
records — separate from core `GraphSnapshot`. As the new core primitives land
(tasks/140-141), mirror the data-in/out ones onto the reader so wasm clients get the
same ergonomics:
- `first_neighbor` / `follow`, `has_edge` / `has_neighbor_with_property`,
  `neighbors_by_types` (deduped), `degree` (O(1) via resident offsets).
- A reader bitmap/`NodeSet` return for `khop`-style expansions (the reader already
  depends on roaring/roaringrange).

Confirm the reader's range-fetched adjacency variant supports these without forcing
full residency. Re-run `wasm-pack build --target web --features wasm` after, and the
round-trip reader tests.

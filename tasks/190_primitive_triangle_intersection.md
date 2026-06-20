# 190 — Primitive: triangle count / sorted-neighbor intersection

Status: PENDING (surfaced by survey; needs the "primitive exercise" + sign-off; coordinate with the
other session — Graphalytics is theirs). Core primitive. Distinct from 188/189.

## Pattern

For each node, count the connected PAIRS among its neighbors (triangles through the node), via
sorted-adjacency INTERSECTION (walk each neighbor's adjacency, intersect against the node's neighbor
set) — NOT by enumerating all C(k,2) pairs. Output: a per-node scalar (triangle count), not a
pair->weight map. The neighbor-set build + gallop/merge intersection is the reusable core.

## Consumers (survey evidence)

- **Graphalytics LCC** `lcc` / `lcc_count_range` (src/graphalytics/mod.rs:261-403) — hand-rolls a
  sorted forward-CSR build (mod.rs:269-282) + bitset-mark + scan/`gallop` intersection
  (mod.rs:334-397, 403) to count rels among N(v). The Graphalytics agent was firm: LCC wants the
  SCALAR triangle count, NOT a pair-emitting co-occurrence (materializing C(k,2) would be strictly
  more expensive). So this is its OWN primitive, distinct from 189.
- **BI Q11** `q11_friend_triangles` (src/bi/faithful_a.rs; python/bi/q11.py) — counts triangles a<b<c
  in a DATE-FILTERED knows subgraph. Note: Q11 filters rels by `kd` window, so the primitive may need
  an optional rel predicate, or Q11 stays partly bespoke. LCC is unfiltered.

## Prior art / naming

Triangle counting; local clustering coefficient; neighbor (set) intersection. Candidates:
`triangles` / `triangle_count` (per node), `count_neighbor_rels`, or a lower-level
`sorted_neighbors` + `intersect` building block. Decide whether to expose the high-level
triangle/LCC count or the reusable sorted-adjacency-intersection building block.

## Design notes

- Output is a per-node scalar (or total) — not pairs. Build the sorted neighbor adjacency once
  (forward CSR), then intersect.
- Rel filter (Q11's kd window) is the divergence; LCC has none. Possibly two consumers only if the
  filter is supported, else LCC alone (coordinate — the other session owns graphalytics and may want
  this for the GA suite directly).

## Next steps
Coordinate with the other session (graphalytics owner). Primitive exercise + sign-off, then build.

## Result
(pending)

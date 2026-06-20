# 191 — Primitive: bulk rel accessor with properties (binding)

Status: PROPOSED — running the "primitive exercise" (CLAUDE.md) before building. The
FinBench optimization pass (parity already proven, 12/12 == Rust) found every gap is
per-rel Python overhead; this is the lever.

## Pattern (the recurring shape)

For a node's incident rels of type R in direction D, get *aligned arrays* of
(neighbor, rel-prop…) in ONE native call — the property-bearing bulk sibling of
`neighbor_ids` (which returns just neighbor ids). Replaces the per-rel
`g.relationships(n, d, [rel])` path (a PyO3 Relationship object + a key-reresolving
`get_property` + `start_node().id()` *per rel*).

## 1. Reusability — consumers

All read a rel property *during traversal*, today via the slow per-rel path:
- **FinBench, all 12 CRs** — transfer/withdraw/deposit/repay/apply `ts`+`amt`. The
  dominant cost (profile of CR1/7/9: ~1.3M `get_property` + ~0.85M `.id()` per 20-run
  batch). CR1 45.5ms vs Rust 1.92ms (~24x), CR9 19x, CR7 15x, CR8 9x, CR3 5x.
- **IC5** (`hasMember` hd), **IC7** (`likes` ld), **IC11** (`workAt` wf).
- **BI Q11** (`knows` kd).
Far past the ≥2-consumer bar — foundational for any "read a rel prop while traversing".

## 2. Prior art

Adjacency-with-attributes: NetworkX `G.edges(n, data=...)` / `G.adj[n][m][attr]`;
igraph incident-edge ids + `g.es[attr]`; Gremlin `outE().valueMap()`. The topology-only
form `neighbor_ids` already exists — this is its property-bearing counterpart.

## 3. Naming + ergonomics (both sides)

**Core: NO new core API** — `relationships_with(node, dir, rel)` (RelationshipRef{neighbor,
pos}) + `rel_col(key)` / `rel_prop(pos, key)` already exist. So this is a BINDING addition
wrapping existing core (precedent: the `dijkstra` binding wrapped core `dijkstra`). The
Rust queries are already fast (hoisted `rel_col`), so they need nothing.

Binding signature: `g.<name>(node, direction, rel_type, prop_keys: list[str]) -> (neighbors, prop_columns)`.
Name candidates: `rels_with_props`, `rel_arrays`, `incident_props` (avoid `rel_props` — collides
with the singular `rel_prop(pos, key)`).

## 4. Hot path / no callbacks

Resolve rel_type + each `rel_col(key)` ONCE; iterate core `relationships_with` in Rust,
read each prop by CSR pos from the hoisted column, box into Python lists. No per-rel
pyclass, no per-rel key reresolution, no Python callback. (GIL is held while building the
lists — the win is killing the object + get_property overhead, not releasing the GIL. A
later native-filter/native-reduce variant could release it, but window/threshold/last_ts
filters are query-specific, so v1 returns arrays and Python filters — per the no-callback rule.)

## 5. Both call sites

Python (CR9 transfer sums):
```python
nbrs, (ts, amt) = g.rels_with_props(account, Direction.Outgoing, "transfer", ["ts", "amt"])
s = sum(a for t, a in zip(ts, amt) if start_ms <= t <= end_ms and a >= threshold)
```
Rust: unchanged (already reads `rel_col` by pos).

## Open decisions (for sign-off)

1. **Return shape:** (A) parallel Python lists `(neighbors, [prop_lists])` — simple, no new
   pyclass, one int/float box per value; ample for FinBench's avg degree ~8. vs (B) a
   `RelView` pyclass exposing `.neighbors` / `.i64(k)` / `.f64(k)` as buffer-protocol arrays
   (zero-copy memoryview, no numpy) — faster for hub nodes, more surface. (Outgoing rel props
   are a contiguous CSR-pos slice → could be truly zero-copy; incoming are scattered → must
   materialize, so B is only partially zero-copy.)
2. **Name:** `rels_with_props` / `rel_arrays` / `incident_props`.

## Result
DONE (2026-06-20, Eve: "do both and compare"). Built BOTH shapes (binding-only, no core change),
main repo `7747605`:
- `rels_with_props(node, dir, rel, keys) -> (neighbors, [value_lists])` — parallel lists, clean zip;
  ~2.6x over relationships() on the hub (1.40→0.70ms gather).
- `rel_view(node, dir, rel, keys) -> RelView` with `.neighbors`/`.col(key)` zero-copy buffer-protocol
  `RelArray`s (format I/q/d); native gather GIL-released, `sum(memoryview(...))` reduces at C speed —
  ~7x on the hub (1.89→0.27ms). New pyclasses RelArray/RelView; test_rels_with_props.py (5).
- Wired each FinBench query to the better fit (ldbc): rel_view+sum(memoryview) for the reductions
  (CR7, CR9, guarantee), rels_with_props for filter/traverse (CR1/2/5/6/8/10/12 + helpers).
  **CR7 12.5→0.9ms (Rust 0.82, parity), CR9 11.0→0.5ms (Rust 0.58, parity/beat), CR3 6.1→3.2ms.**
  CR1 45.5→37.4ms (still ~19x — it's filter+BFS+sort in Python, not a pure reduction; closing it needs
  a native filter/traverse kernel, a bigger primitive). All 12 still EXACT-match Rust; 13 fixture tests
  green. GOTCHA fixed: cr1's signIn must be per-rel (rels_with_props([]) — Rust counts per signIn rel,
  duplicates included; neighbor_ids deduped → 605→133 regression).

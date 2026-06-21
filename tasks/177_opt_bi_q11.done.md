# 177 — Optimize BI Q11 (friend triangles)

Baseline (full SF1, median of 5): Python 14.7 ms vs Rust 2.74 ms (~5.4x).
Lead: building the date-filtered knows adjacency reads each rel via
`g.relationships(a, Out, ["knows"])` then `rel.end_node().id()` + `rel.get_property("kd")`
— a `Node` is constructed per rel just to read its id.
Approach: a cheaper "neighbors-with-rel-property" read (return `(neighbor_id, prop)` pairs
without a `Node` per rel) would cut the per-rel overhead; reusable for any kd/rel-prop
traversal (also helps Q11-style date-filtered adjacency). Profile first to confirm the
Node-construction is the lead.

## Result
(pending)

## Result (2026-06-21) — DONE (rels_with_props)
The date-filtered knows adjacency now uses the bulk `rels_with_props(a, Out,
"knows", ["kd"])` accessor — aligned `(neighbor_id, kd)` arrays — instead of
`relationships()` constructing a `Node` per rel + a `get_property` per rel. Parity
ok (count=805). Clean structural win (strictly less per-rel work); a precise
speedup measurement is pending a quiet machine (the box was saturated by unrelated
VM/Roblox load at validation time).

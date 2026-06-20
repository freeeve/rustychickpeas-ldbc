# 188 — Primitive: rel fold (one-mode projection) — `fold_via`

Status: PENDING (justified by cross-suite survey; needs the "primitive exercise" + sign-off
before building). Core/Python primitive. See CLAUDE.md "primitive exercise" checklist.

## Pattern (kernel A)

For each rel of relationship R in direction D, project BOTH endpoints through a follow-chain
to derived nodes a'=f(a), b'=f(b), and accumulate a weight into the (undirected) pair (a', b'),
skipping self-pairs. Output: a `(a', b') -> weight` map. = one-mode / rel projection / line-graph
folding.

Canonical: for each `replyOf` rel (comment -> parent), project both to their creator via
`hasCreator`; count per person-pair -> a person<->person interaction graph.

## Consumers (survey evidence)

- **BI Q19** `build_interaction_map` (src/bi/faithful_b.rs:113-130; python/bi/q19.py) — replyOf ->
  creator, COUNT. The ~1.8s Python build is the lever.
- **IC14** `build_knows_interaction` (src/interactive.rs:195-212) — SAME replyOf->creator COUNT
  fold; its doc says it "mirrors the BI Q19 projection."
- **BI Q15** `_build_weights` (python/bi/q15.py) — replyOf -> creator, but a CONDITIONAL/VALUED
  weight (+1.0 if parent is a Post else +0.5, only if the thread-root forum's fday in window).
  -> needs a weight SPEC, not just count; otherwise Q15 stays at its ~700ms Python-loop floor.

So 2 clean count-consumers (Q19, IC14) + Q15 (weighted). Clears the ">=2 real consumers" bar.

## Prior art / naming

One-mode / bipartite / rel projection; "network folding"; line graph. NetworkX
`bipartite.*projected_graph`, igraph `bipartite_projection`. NOTE: "project" is already taken
(`neighbor_groups(...).project(...)` = follow-chain) and "fold" collides with FP reduce -> settle
the name in the exercise. Candidates: `fold_via`, `projected_pairs`, `interactions_via`.

## Ergonomics sketch (count version)

```python
interaction = g.fold_via("replyOf", Direction.Outgoing, project=[(Direction.Incoming, "hasCreator")])
# -> {(min_person, max_person): count}, self-pairs skipped
```
```rust
let m = g.fold_via("replyOf", Direction::Outgoing, &[(Direction::Incoming, "hasCreator")]);
// HashMap<(u32,u32), u64>
```
- Parallel core kernel (par_fold over rels/comments). Same `project` follow-chain as neighbor_groups,
  applied to BOTH endpoints.
- Output: ~797k pairs for Q19 -> Python dict materialization ~200ms (still a big win vs 1.8s).
- Weight modes to consider: count; sum-of-rel-property; and (for Q15) a declarative valued/filtered
  weight — but Q15's filter is "window on a 2-hop-projected node's property," which may be too bespoke
  (don't over-engineer; NO Python callback in the kernel per CLAUDE.md rule). Likely: ship count/sum,
  leave Q15's conditional weight at the Python floor.

## Next steps
Run the primitive exercise (naming + both-sides ergonomics + sign-off), then build the core kernel +
binding, wire Q19 + IC14 (and Q15 if a weight spec lands).

## Result
DONE (2026-06-19, Eve signed off on the primitive exercise — name `fold_via`, projection =
precomputed `NodeArray`, count-only).

Built in the `rustychickpeas` core repo (gated commits, NOT pushed):
- Core `GraphSnapshot::fold_via(rel, direction, projection: &[NodeId]) -> HashMap<(NodeId,NodeId),u64>`
  in `graph_snapshot.rs`: for each `rel` rel a->b in `direction`, add 1 to the unordered pair
  `(min,max)` of `projection[a]`/`projection[b]`; self-pairs and `u32::MAX` (no-neighbor) endpoints
  skipped. Parallel: rayon `par_iter` over `0..n_nodes`, thread-local maps merged (smaller into
  larger) on `reduce`. Unit test `test_fold_via`.
- Binding `GraphSnapshot.fold_via(rel, direction, projection: NodeArray) -> {(a,b): count}` — clones
  the `NodeArray`'s `Arc<[u32]>`, runs the core kernel under `allow_threads` (GIL released), returns
  a tuple-keyed dict. `tests/test_fold_via.py` (3). The projection comes from `neighbor_via`/`roots_via`.

Wired BI Q19 `build_interaction_map` → two lines:
`creators = g.neighbor_via("hasCreator", Incoming); g.fold_via("replyOf", Outgoing, creators)`.
replyOf rels originate only from Comments, so folding every node == the old per-Comment scan.
**Result: precompute (Q19 map + Q20 weights) 1.8s → 0.2s; 20/20 BI parity preserved; Q19 still the
exact 6-row match.** The native parallel kernel collapsed the documented biggest single BI build lever.

NOTE: the remaining Q19 *query* cost (~735ms) is the per-city1-person heap Dijkstra — a separate
optimization (task 185, query-side bidirectional Dijkstra), NOT what fold_via targets.

FOLLOW-UP (deferred, trivial): wire IC14 `build_knows_interaction` (src/interactive.rs) — the
identical replyOf->creator count fold — onto core `fold_via` to dedupe the Rust. IC isn't ported to
Python (BI is the active workstream), so adopt when IC work resumes. Q15's conditional/windowed weight
stays at the Python floor (can't express its 2-hop-projected forum-fday window without a per-rel
Python callback, forbidden in the kernel) — see `181_opt_bi_q15.done.md`.

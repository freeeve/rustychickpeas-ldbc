# 188 — Primitive: edge fold (one-mode projection) — `fold_via`

Status: PENDING (justified by cross-suite survey; needs the "primitive exercise" + sign-off
before building). Core/Python primitive. See CLAUDE.md "primitive exercise" checklist.

## Pattern (kernel A)

For each edge of relationship R in direction D, project BOTH endpoints through a follow-chain
to derived nodes a'=f(a), b'=f(b), and accumulate a weight into the (undirected) pair (a', b'),
skipping self-pairs. Output: a `(a', b') -> weight` map. = one-mode / edge projection / line-graph
folding.

Canonical: for each `replyOf` edge (comment -> parent), project both to their creator via
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

One-mode / bipartite / edge projection; "network folding"; line graph. NetworkX
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
- Parallel core kernel (par_fold over edges/comments). Same `project` follow-chain as neighbor_groups,
  applied to BOTH endpoints.
- Output: ~797k pairs for Q19 -> Python dict materialization ~200ms (still a big win vs 1.8s).
- Weight modes to consider: count; sum-of-edge-property; and (for Q15) a declarative valued/filtered
  weight — but Q15's filter is "window on a 2-hop-projected node's property," which may be too bespoke
  (don't over-engineer; NO Python callback in the kernel per CLAUDE.md rule). Likely: ship count/sum,
  leave Q15's conditional weight at the Python floor.

## Next steps
Run the primitive exercise (naming + both-sides ergonomics + sign-off), then build the core kernel +
binding, wire Q19 + IC14 (and Q15 if a weight spec lands).

## Result
(pending)

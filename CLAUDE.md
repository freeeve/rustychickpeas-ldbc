# Repository conventions

## Terminology: node / rel, not vertex / edge

Prefer **node** and **rel** (relationship) wording over **vertex** and **edge** in
code, identifiers, comments, and docs — to match the core library's `NodeId` /
`RelationshipType` vocabulary and keep the benchmark client consistent with core.

- The LDBC Graphalytics spec uses "vertex" / "edge"; translate to node / rel in our
  code. Keep the spec's terms only when directly quoting it (e.g. a doc comment
  citing the algorithm definition).
- `src/graphalytics/` algorithm code (`mod.rs`) and the bin are aligned to node/rel
  (tasks/145). The vertex/edge wording that remains there is deliberate: `load.rs`
  and `validate.rs` use **vertex** for the *original dataset id* (as distinct from the
  dense node id) — e.g. `vertex_of_node` / `node_of_vertex`, the `.v`/`.e` file format,
  the `source-vertex` config keys, and `<vertex-id> <value>` reference files. Keep that
  sense of "vertex"; it is the dataset/spec vocabulary, not our graph's nodes.

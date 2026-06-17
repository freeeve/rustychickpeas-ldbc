# Repository conventions

## Terminology: node / rel, not vertex / edge

Prefer **node** and **rel** (relationship) wording over **vertex** and **edge** in
code, identifiers, comments, and docs — to match the core library's `NodeId` /
`RelationshipType` vocabulary and keep the benchmark client consistent with core.

- The LDBC Graphalytics spec uses "vertex" / "edge"; translate to node / rel in our
  code. Keep the spec's terms only when directly quoting it (e.g. a doc comment
  citing the algorithm definition).
- `src/graphalytics/` still uses vertex/edge wording in places — align it to
  node/rel when next touched.

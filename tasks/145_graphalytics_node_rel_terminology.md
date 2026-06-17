# 145 — Graphalytics: node/rel terminology (per CLAUDE.md)

Align `src/graphalytics/` + `src/bin/graphalytics.rs` to the repo convention
(CLAUDE.md): prefer node/rel over vertex/edge.

- `mod.rs` (algorithms): "vertex"/"vertices" → "node"/"nodes" in comments/docs where
  it means our dense nodes; "edge" → "rel" where it's our relationship. Keep "edge"
  only in spec-quoting doc comments.
- **Keep** the loader's `vertex_of_node` / `node_of_vertex` maps and the .v/.e
  "vertex"/"edge" wording in `load.rs` + `validate.rs`: there "vertex" means the
  **original dataset id** as distinct from the dense node id — a meaningful
  distinction, and the dataset/spec vocabulary.
- Update the bin's "Loaded N vertices" line → nodes.

Mechanical; verify with `cargo test --lib graphalytics` (16 tests) + a wiki-Talk run.

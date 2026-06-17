# 124 — Graphalytics WCC (implementation)

Implement `graphalytics::wcc` to the LDBC spec, on the loaded GraphSnapshot
(Direction::Outgoing for directed graphs, Both for undirected unless noted).

Spec: weakly connected components (Direction::Both); component id = min vertex id in the component. validated by equivalence/relabel.

Validate with a unit test on a small hand-computed graph (built via GraphBuilder).

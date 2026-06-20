# 127 — Graphalytics SSSP (implementation)

Implement `graphalytics::sssp` to the LDBC spec, on the loaded GraphSnapshot
(Direction::Outgoing for directed graphs, Both for undirected unless noted).

Spec: weighted SSSP over OUTGOING rels (undirected: both), sum of weights. unreachable=f64::INFINITY. wrap core g.dijkstra, weight=g.relationship_property(rel.pos,"weight").

Validate with a unit test on a small hand-computed graph (built via GraphBuilder).

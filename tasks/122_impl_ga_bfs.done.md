# 122 — Graphalytics BFS (implementation)

Implement `graphalytics::bfs` to the LDBC spec, on the loaded GraphSnapshot
(Direction::Outgoing for directed graphs, Both for undirected unless noted).

Spec: depth/hop count from source over OUTGOING edges (undirected: both). root=0. unreachable = 9223372036854775807 (i64). reuse core g.bfs_distances.

Validate with a unit test on a small hand-computed graph (built via GraphBuilder).

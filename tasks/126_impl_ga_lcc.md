# 126 — Graphalytics LCC (implementation)

Implement `graphalytics::lcc` to the LDBC spec, on the loaded GraphSnapshot
(Direction::Outgoing for directed graphs, Both for undirected unless noted).

Spec: 0 if |N(v)|<=1 else (sum_{u in N(v)} |N(v) intersect Nout(u)|)/(|N(v)|*(|N(v)|-1)). N(v)=undirected neighbour set, each once; Nout directed. f64.

Validate with a unit test on a small hand-computed graph (built via GraphBuilder).

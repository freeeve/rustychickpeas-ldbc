# 123 — Graphalytics PAGERANK (implementation)

Implement `graphalytics::pagerank` to the LDBC spec, on the loaded GraphSnapshot
(Direction::Outgoing for directed graphs, Both for undirected unless noted).

Spec: PR0(v)=1/|V|. PRi(v)=(1-d)/|V| + d*(sum_{u in Nin(v)} PRi-1(u)/|Nout(u)| + sum_{sink w} PRi-1(w)/|V|). sinks=|Nout|=0. fixed max_iterations. f64.

Validate with a unit test on a small hand-computed graph (built via GraphBuilder).

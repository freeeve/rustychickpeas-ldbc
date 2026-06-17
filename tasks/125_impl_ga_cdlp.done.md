# 125 — Graphalytics CDLP (implementation)

Implement `graphalytics::cdlp` to the LDBC spec, on the loaded GraphSnapshot
(Direction::Outgoing for directed graphs, Both for undirected unless noted).

Spec: L0(v)=v. synchronous: Li(v)=min(argmax label frequency over Nin(v)+Nout(v), mutual neighbour counted twice; smallest label on tie; no neighbours -> keep). fixed max_iterations.

Validate with a unit test on a small hand-computed graph (built via GraphBuilder).

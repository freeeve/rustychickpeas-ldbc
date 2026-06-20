"""SPB hand-coded queries over the RDF property graph (no SPARQL), ported from
src/spb/{q,a}*.rs. Params come from the parity param set; results are compared by
node ``uri`` (or (key,count) rows for aggregates).

Helpers: ``node_by_uri`` resolves a param IRI via the loader's uri->node map;
``neighbors_in_set`` = a neighbor traversal filtered to a label set (the binding
has no neighbors_in_set, so it's a Python set intersection).
"""

from rustychickpeas import Direction


def node_by_uri(uri_map, uri):
    return uri_map.get(uri)


def neighbors_in_set(g, node, direction, pred, node_set):
    return [n for n in g.neighbor_ids(node, direction, [pred]) if n in node_set]


def q1(g, uri_map, topic_uri):
    """SPB q1 — creative works that are about/mentions a thing, newest-modified first.
    Returns work node ids ordered (dateModified desc, id asc)."""
    topic = uri_map.get(topic_uri)
    if topic is None:
        return []
    cworks = set(g.nodes_with_label("CreativeWork"))
    works = set()
    for pred in ("about", "mentions"):
        works.update(neighbors_in_set(g, topic, Direction.Incoming, pred, cworks))
    rows = [(w, g.prop_str(w, "dateModified")) for w in works]
    rows = [(w, d) for (w, d) in rows if d is not None]
    rows.sort(key=lambda r: r[0])                 # id asc
    rows.sort(key=lambda r: r[1], reverse=True)   # then dateModified desc (stable)
    return [w for (w, _) in rows]


def a9(g):
    """SPB a9 — the largest number of outgoing ``mentions`` on any CreativeWork."""
    works = g.nodes_with_label("CreativeWork")
    best = 0
    for w in works:
        c = len(g.neighbor_ids(w, Direction.Outgoing, ["mentions"]))
        if c > best:
            best = c
    return best

"""Cypher-to-Cypher parity: run identical Cypher through the RustyChickpeas engine
(``snapshot.cypher``) and Kuzu over the same synthetic BI-shaped graph, and assert
the result sets match.

Covers the engine features the LDBC BI workload leans on that the read-only Cypher
engine currently supports: label-scoped traversal + aggregation, WITH chaining with
a post-aggregate filter (HAVING), variable-length paths, EXISTS subqueries, and CASE.

Run only this file (it needs both ``rustychickpeas`` with ``.cypher`` and ``kuzu`` in
one venv; the LDBC ``.venv-kuzu`` has both):

    .venv-kuzu/bin/python -m pytest python/tests/test_bi_cypher_parity.py
"""

import kuzu
import pytest
from rustychickpeas import GraphSnapshotBuilder, RustyChickpeas

# (id, name, age, city)
PEOPLE = [
    (0, "Alice", 30, "London"),
    (1, "Bob", 35, "London"),
    (2, "Carol", 40, "Paris"),
    (3, "Dave", 25, "Paris"),
    (4, "Eve", 28, "London"),
]
FORUMS = [(5, "Rust"), (6, "Graphs")]
TAGS = [(7, "db"), (8, "ml")]
KNOWS = [(0, 1), (0, 2), (1, 2), (1, 3), (2, 3), (2, 1), (3, 4), (4, 0)]
HAS_MEMBER = [(5, 0), (5, 1), (5, 2), (6, 2), (6, 3)]  # Forum -> Person
HAS_INTEREST = [(0, 7), (1, 7), (2, 8), (3, 7)]  # Person -> Tag (Eve has none)

QUERIES = [
    # members per forum
    "MATCH (f:Forum)-[:hasMember]->(p:Person) "
    "RETURN f.title AS forum, count(*) AS members ORDER BY members DESC, forum",
    # friends-of count with a HAVING-style WITH filter
    "MATCH (p:Person)-[:KNOWS]->(:Person) WITH p, count(*) AS c WHERE c >= 2 "
    "RETURN p.name AS name, c ORDER BY name",
    # variable-length reachability
    "MATCH (p:Person {name: 'Alice'})-[:KNOWS*1..2]->(f:Person) "
    "RETURN DISTINCT f.name AS name ORDER BY name",
    # NOT EXISTS subquery: people with no interests
    "MATCH (p:Person) WHERE NOT EXISTS { MATCH (p)-[:hasInterest]->(:Tag) } "
    "RETURN p.name AS name ORDER BY name",
    # CASE bucketing + count
    "MATCH (p:Person) "
    "RETURN CASE WHEN p.age < 30 THEN 'young' ELSE 'old' END AS bucket, count(*) AS c "
    "ORDER BY bucket",
    # interest popularity
    "MATCH (t:Tag)<-[:hasInterest]-(p:Person) "
    "RETURN t.name AS tag, count(*) AS fans ORDER BY fans DESC, tag",
]


@pytest.fixture(scope="module")
def rcp_graph():
    """The synthetic graph in a RustyChickpeas snapshot."""
    b = GraphSnapshotBuilder(version="bi-parity")
    for i, name, age, city in PEOPLE:
        b.add_node(["Person"], node_id=i)
        b.set_prop(i, "name", name)
        b.set_prop(i, "age", age)
        b.set_prop(i, "city", city)
    for i, title in FORUMS:
        b.add_node(["Forum"], node_id=i)
        b.set_prop(i, "title", title)
    for i, name in TAGS:
        b.add_node(["Tag"], node_id=i)
        b.set_prop(i, "name", name)
    for u, v in KNOWS:
        b.add_relationship(u, v, "KNOWS")
    for u, v in HAS_MEMBER:
        b.add_relationship(u, v, "hasMember")
    for u, v in HAS_INTEREST:
        b.add_relationship(u, v, "hasInterest")

    mgr = RustyChickpeas()
    b.finalize_into(mgr)
    return mgr.graph_snapshot(mgr.versions()[0])


@pytest.fixture(scope="module")
def kuzu_conn(tmp_path_factory):
    """The same graph in an in-process Kuzu database."""
    db = kuzu.Database(str(tmp_path_factory.mktemp("kuzu") / "db"))
    conn = kuzu.Connection(db)
    conn.execute("CREATE NODE TABLE Person(id INT64, name STRING, age INT64, city STRING, PRIMARY KEY(id))")
    conn.execute("CREATE NODE TABLE Forum(id INT64, title STRING, PRIMARY KEY(id))")
    conn.execute("CREATE NODE TABLE Tag(id INT64, name STRING, PRIMARY KEY(id))")
    conn.execute("CREATE REL TABLE KNOWS(FROM Person TO Person)")
    conn.execute("CREATE REL TABLE hasMember(FROM Forum TO Person)")
    conn.execute("CREATE REL TABLE hasInterest(FROM Person TO Tag)")
    for i, name, age, city in PEOPLE:
        conn.execute(f"CREATE (:Person {{id:{i}, name:'{name}', age:{age}, city:'{city}'}})")
    for i, title in FORUMS:
        conn.execute(f"CREATE (:Forum {{id:{i}, title:'{title}'}})")
    for i, name in TAGS:
        conn.execute(f"CREATE (:Tag {{id:{i}, name:'{name}'}})")
    for tbl, edges in (("KNOWS", KNOWS), ("hasMember", HAS_MEMBER), ("hasInterest", HAS_INTEREST)):
        a, b_ = {"KNOWS": ("Person", "Person"), "hasMember": ("Forum", "Person"), "hasInterest": ("Person", "Tag")}[tbl]
        for u, v in edges:
            conn.execute(
                f"MATCH (x:{a} {{id:{u}}}), (y:{b_} {{id:{v}}}) CREATE (x)-[:{tbl}]->(y)"
            )
    return conn


def _norm(rows):
    """Normalize a result set to a sorted list of value tuples (order-insensitive,
    floats rounded) so the two engines can be compared regardless of row order."""
    def cell(v):
        return round(v, 6) if isinstance(v, float) else v

    return sorted(tuple(cell(c) for c in row) for row in rows)


def _rcp_rows(g, query):
    # cypher() returns dict rows keyed by RETURN column, in column order.
    return [tuple(row.values()) for row in g.cypher(query)]


def _kuzu_rows(conn, query):
    res = conn.execute(query)
    rows = []
    while res.has_next():
        rows.append(tuple(res.get_next()))
    return rows


@pytest.mark.parametrize("query", QUERIES, ids=[f"q{i}" for i in range(len(QUERIES))])
def test_cypher_matches_kuzu(rcp_graph, kuzu_conn, query):
    ours = _norm(_rcp_rows(rcp_graph, query))
    theirs = _norm(_kuzu_rows(kuzu_conn, query))
    assert ours == theirs, f"\nquery: {query}\n ours: {ours}\nkuzu: {theirs}"

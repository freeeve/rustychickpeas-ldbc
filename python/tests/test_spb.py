"""Tests for the SPB RDF -> property-graph loader on a small N-Triples fixture."""

import os
import tempfile

from spb import loader
from spb.loader import parse_line, local_name
from rustychickpeas import Direction

RDF_TYPE = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type"
SUBCLASS = "http://www.w3.org/2000/01/rdf-schema#subClassOf"
SUBPROP = "http://www.w3.org/2000/01/rdf-schema#subPropertyOf"
XSD_INT = "http://www.w3.org/2001/XMLSchema#integer"
XSD_DBL = "http://www.w3.org/2001/XMLSchema#double"

W1 = "http://www.bbc.co.uk/things/1#id"
NEWS = "http://www.bbc.co.uk/ontologies/creativework/NewsItem"
CW = "http://www.bbc.co.uk/ontologies/creativework/CreativeWork"
ABOUT = "http://www.bbc.co.uk/ontologies/creativework/about"
TAG = "http://www.bbc.co.uk/ontologies/creativework/tag"
TITLE = "http://www.bbc.co.uk/ontologies/creativework/title"
WC = "http://www.bbc.co.uk/ontologies/creativework/wordCount"
THING_A = "http://dbpedia.org/resource/A"

_FIXTURE = "\n".join([
    f"<{NEWS}> <{SUBCLASS}> <{CW}> .",
    f"<{ABOUT}> <{SUBPROP}> <{TAG}> .",
    f"<{W1}> <{RDF_TYPE}> <{NEWS}> .",
    f'<{W1}> <{TITLE}> "Hello world" .',
    f'<{W1}> <{WC}> "42"^^<{XSD_INT}> .',
    f"<{W1}> <{ABOUT}> <{THING_A}> .",
])


def test_local_name():
    assert local_name(RDF_TYPE) == "type"
    assert local_name("http://dbpedia.org/ontology/Place") == "Place"
    assert local_name("http://ex/geo#long") == "long"


def test_parse_line():
    s, p, o = parse_line(f'<{W1}> <{TITLE}> "Hi"@en .')
    assert s == ("iri", W1) and p == ("iri", TITLE) and o == ("lit", "Hi", None, "en")
    s, p, o = parse_line(f'<{W1}> <{WC}> "42"^^<{XSD_INT}> .')
    assert o == ("lit", "42", XSD_INT, None)
    assert parse_line("# comment") is None
    assert parse_line("") is None


def _load(text):
    with tempfile.TemporaryDirectory() as tmp:
        path = os.path.join(tmp, "f.nt")
        with open(path, "w", encoding="utf-8") as f:
            f.write(text)
        return loader.load_ntriples(path)


def test_loader_labels_rels_props():
    g, stats = _load(_FIXTURE)
    news = list(g.nodes_with_label("NewsItem"))
    assert len(news) == 1
    w1 = news[0]
    # subClassOf forward-chains the CreativeWork label onto the NewsItem.
    assert w1 in set(g.nodes_with_label("CreativeWork"))
    assert g.get_property(w1, "uri") == W1
    assert g.get_property(w1, "title") == "Hello world"
    assert g.get_property(w1, "wordCount") == 42          # xsd:integer -> int
    # about rel + the subPropertyOf-chained tag rel both point at thing A.
    about = g.neighbor_ids(w1, Direction.Outgoing, ["about"])
    tag = g.neighbor_ids(w1, Direction.Outgoing, ["tag"])
    assert len(about) == 1 and about == tag
    assert g.get_property(about[0], "uri") == THING_A


def test_loader_typed_double_and_first_literal_wins():
    g, _ = _load("\n".join([
        f'<{W1}> <{RDF_TYPE}> <{CW}> .',
        f'<{W1}> <http://ex/geo#lat> "40.86667"^^<{XSD_DBL}> .',
        f'<{W1}> <{TITLE}> "first" .',
        f'<{W1}> <{TITLE}> "second" .',          # first literal per (node,key) wins
    ]))
    w1 = list(g.nodes_with_label("CreativeWork"))[0]
    assert abs(g.get_property(w1, "lat") - 40.86667) < 1e-9
    assert g.get_property(w1, "title") == "first"

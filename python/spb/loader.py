"""SPB RDF -> property-graph loader (mirrors src/spb/loader.rs).

Maps N-Triples into a rustychickpeas GraphSnapshot:
  * every IRI/blank subject or IRI object -> one node (with its IRI as ``uri``);
  * rdf:type -> the object's local name as a **label** (+ super-class local names
    via transitively-closed rdfs:subClassOf, minus owl:Thing);
  * a predicate with an IRI/blank object -> a typed **rel** (predicate local name)
    + a rel for each super-property (transitively-closed rdfs:subPropertyOf);
  * a predicate with a literal object -> a node **property** (predicate local name),
    typed from the literal's xsd: datatype (first value per (node,key) wins).
No triple store, no SPARQL.
"""

import gc
import glob
import os

from rustychickpeas import GraphSnapshotBuilder

RDF_TYPE = "http://www.w3.org/1999/02/22-rdf-syntax-ns#type"
RDFS_SUBCLASS = "http://www.w3.org/2000/01/rdf-schema#subClassOf"
RDFS_SUBPROP = "http://www.w3.org/2000/01/rdf-schema#subPropertyOf"
OWL_THING = "http://www.w3.org/2002/07/owl#Thing"

_INT_XSD = {"integer", "int", "long", "short", "byte", "nonNegativeInteger", "positiveInteger"}
_FLOAT_XSD = {"double", "float", "decimal"}


def local_name(iri):
    """The substring after the last '#' or '/'."""
    return iri.rsplit("#", 1)[-1].rsplit("/", 1)[-1]


def _percent_decode(s):
    if "%" not in s:
        return s
    bs = s.encode("utf-8")
    out = bytearray()
    i = 0
    while i < len(bs):
        if bs[i] == 0x25 and i + 2 < len(bs):  # '%'
            try:
                out.append(int(bs[i + 1:i + 3].decode("ascii"), 16))
                i += 3
                continue
            except ValueError:
                pass
        out.append(bs[i])
        i += 1
    try:
        return out.decode("utf-8")
    except UnicodeDecodeError:
        return s


def _unescape_iri(s):
    if "\\" not in s:
        return s
    out = []
    i = 0
    while i < len(s):
        if s[i] == "\\" and i + 1 < len(s) and s[i + 1] in "uU":
            n = 4 if s[i + 1] == "u" else 8
            hexs = s[i + 2:i + 2 + n]
            try:
                out.append(chr(int(hexs, 16)))
                i += 2 + n
                continue
            except (ValueError, OverflowError):
                pass
        out.append(s[i])
        i += 1
    return "".join(out)


_NAME = set("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_-.")
_LANG = set("abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789-")
_LIT_ESC = {'"': '"', "\\": "\\", "n": "\n", "t": "\t", "r": "\r"}


def _read_term(s, i):
    """Return (term, next_i) or (None, i). term = ('iri',v)/('blank',v)/('lit',v,dt,lang)."""
    n = len(s)
    while i < n and s[i] in " \t":
        i += 1
    if i >= n:
        return None, i
    c = s[i]
    if c == "<":
        end = s.find(">", i + 1)
        if end < 0:
            return None, i
        return ("iri", _unescape_iri(s[i + 1:end])), end + 1
    if c == "_":
        if s[i + 1:i + 2] != ":":
            return None, i
        j = i + 2
        while j < n and s[j] in _NAME:
            j += 1
        return ("blank", s[i + 2:j]), j
    if c == '"':
        return _read_literal(s, i)
    return None, i


def _read_literal(s, i):
    n = len(s)
    i += 1
    buf = []
    while True:
        if i >= n:
            return None, i
        c = s[i]
        if c == "\\":
            i += 1
            if i >= n:
                return None, i
            e = s[i]
            if e == "u":
                buf.append(chr(int(s[i + 1:i + 5], 16)))
                i += 4
            elif e == "U":
                buf.append(chr(int(s[i + 1:i + 9], 16)))
                i += 8
            else:
                buf.append(_LIT_ESC.get(e, e))
            i += 1
        elif c == '"':
            i += 1
            break
        else:
            buf.append(c)
            i += 1
    value = "".join(buf)
    datatype = lang = None
    if s[i:i + 2] == "^^":
        i += 2
        t, i = _read_term(s, i)
        if t is None or t[0] != "iri":
            return None, i
        datatype = t[1]
    elif s[i:i + 1] == "@":
        i += 1
        j = i
        while j < n and s[j] in _LANG:
            j += 1
        lang = s[i:j]
        i = j
    return ("lit", value, datatype, lang), i


def _node_term(tok):
    """Parse a subject/predicate token (an <iri> or _:blank — no internal spaces)."""
    if tok.startswith("<") and tok.endswith(">"):
        return ("iri", _unescape_iri(tok[1:-1]))
    if tok.startswith("_:"):
        return ("blank", tok[2:])
    return None


def parse_line(line):
    """Parse one N-Triples line into (subject, predicate, object) terms, or None.

    Fast path: subject and predicate are space-free (`<iri>` / `_:blank`), so split
    off the first two tokens and treat the rest (minus the trailing ` .`) as the
    object — which may be an IRI, blank, or a literal (with internal spaces)."""
    line = line.strip()
    if not line or line[0] == "#":
        return None
    if line.endswith("."):
        line = line[:-1].rstrip()
    parts = line.split(" ", 2)
    if len(parts) < 3:
        return None
    s = _node_term(parts[0])
    p = _node_term(parts[1])
    if s is None or p is None or p[0] != "iri":
        return None
    obj = parts[2]
    if obj.startswith('"'):
        o = _fast_literal(obj)
    else:
        o = _node_term(obj.rstrip())
    if o is None:
        return None
    return s, p, o


def _fast_literal(tok):
    """Parse an object literal token. Fast path (no backslash): the closing quote is
    the next `"`, so slice the value and read any ^^<dt> / @lang suffix without a
    char loop. Falls back to the escape-aware reader when a backslash is present."""
    if "\\" in tok:
        term, _ = _read_literal(tok, 0)
        return term
    end = tok.find('"', 1)
    if end < 0:
        return None
    value = tok[1:end]
    rest = tok[end + 1:].rstrip()
    if rest.startswith("^^<") and rest.endswith(">"):
        return ("lit", value, rest[3:-1], None)
    if rest.startswith("@"):
        return ("lit", value, None, rest[1:])
    return ("lit", value, None, None)


def _close_transitively(m):
    changed = True
    while changed:
        changed = False
        for k in list(m.keys()):
            for d in list(m[k]):
                for sup in m.get(d, ()):
                    if sup not in m[k]:
                        m[k].add(sup)
                        changed = True


def _resource_key(term):
    if term[0] == "iri":
        return "I:" + _percent_decode(term[1])
    if term[0] == "blank":
        return "B:" + term[1]
    return None


def _iter_triples(paths):
    for path in paths:
        with open(path, "r", encoding="utf-8") as fh:
            for line in fh:
                t = parse_line(line)
                if t is not None:
                    yield t


def load_ntriples(paths):
    """Build the SPB property graph from N-Triples file(s). Returns (snapshot, stats)."""
    gc.disable()  # millions of short-lived triple tuples -> GC churn dominates; load is bulk-allocate
    try:
        return _load_ntriples(paths)
    finally:
        gc.enable()


def _load_ntriples(paths):
    if isinstance(paths, str):
        paths = [paths]
    triples = list(_iter_triples(paths))

    # TBox: subClassOf / subPropertyOf, transitively closed.
    subclass, subprop = {}, {}
    for s, p, o in triples:
        if s[0] == "iri" and o[0] == "iri":
            if p[1] == RDFS_SUBCLASS:
                subclass.setdefault(s[1], set()).add(o[1])
            elif p[1] == RDFS_SUBPROP:
                subprop.setdefault(s[1], set()).add(o[1])
    _close_transitively(subclass)
    _close_transitively(subprop)

    def is_tbox(p):
        return p[1] in (RDFS_SUBCLASS, RDFS_SUBPROP)

    # Pass 1: intern resources -> ids; collect rdf:type IRIs + uris.
    ids, types, uri_of = {}, {}, {}

    def intern(term):
        key = _resource_key(term)
        nid = ids.get(key)
        if nid is None:
            nid = len(ids)
            ids[key] = nid
            if term[0] == "iri":
                uri_of[nid] = _percent_decode(term[1])
        return nid

    for s, p, o in triples:
        if is_tbox(p):
            continue
        sid = intern(s)
        if p[1] == RDF_TYPE:
            if o[0] == "iri":
                types.setdefault(sid, []).append(o[1])
        elif o[0] in ("iri", "blank"):
            intern(o)

    n = len(ids)
    b = GraphSnapshotBuilder(capacity_nodes=n + 1, capacity_rels=len(triples) + 1)

    # Pass 2: nodes with labels (type + super-class local names, minus owl:Thing) + uri.
    for nid in range(n):
        label_iris = set()
        for ty in types.get(nid, ()):
            label_iris.add(ty)
            for sup in subclass.get(ty, ()):
                if sup != OWL_THING:
                    label_iris.add(sup)
        labels = [local_name(iri) for iri in label_iris]
        b.add_node(labels, node_id=nid)
        uri = uri_of.get(nid)
        if uri is not None:
            b.set_prop(nid, "uri", uri)

    # Pass 3: rels (IRI objects + super-properties) and properties (literals, first wins).
    stats = {"resources": n, "triples": len(triples), "rels": 0, "literals": 0}
    seen_props = set()
    for s, p, o in triples:
        if p[1] == RDF_TYPE or is_tbox(p):
            continue
        subj = ids[_resource_key(s)]
        key = local_name(p[1])
        if o[0] in ("iri", "blank"):
            dst = ids[_resource_key(o)]
            b.add_relationship(subj, dst, key)
            stats["rels"] += 1
            for sup in subprop.get(p[1], ()):
                b.add_relationship(subj, dst, local_name(sup))
        elif o[0] == "lit":
            pk = (subj, key)
            if pk not in seen_props:
                seen_props.add(pk)
                b.set_prop(subj, key, _literal_value(o))
                stats["literals"] += 1

    # uri -> node id (free from uri_of) so queries can resolve param uris without a
    # label-free property lookup (the binding only exposes node_with_label_property).
    stats["uri_to_node"] = {uri: nid for nid, uri in uri_of.items()}
    return b.finalize(), stats


def _literal_value(lit):
    """Typed value for a literal term per its xsd datatype (else string)."""
    _, value, datatype, _lang = lit
    if datatype is not None:
        dt = local_name(datatype)
        if dt in _INT_XSD:
            try:
                return int(value)
            except ValueError:
                pass
        elif dt in _FLOAT_XSD:
            try:
                return float(value)
            except ValueError:
                pass
        elif dt == "boolean":
            if value in ("true", "false"):
                return value == "true"
    return value

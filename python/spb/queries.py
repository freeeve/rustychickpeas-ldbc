"""SPB hand-coded queries over the RDF property graph (no SPARQL), ported from
src/spb/{q,a}*.rs. Params come from the parity param set; results are compared by
node ``uri`` (or aggregate rows) against results/spb.parity.rust.json.

Helpers
-------
* ``node_by_uri`` resolves a param IRI via the loader's uri->node map (the binding
  has no label-free property lookup).
* ``neighbors_in_set`` = a typed neighbour traversal filtered to a node set (the
  binding has no ``neighbors_in_set``, so it's a Python set intersection). Since an
  RDF triple set has no duplicate triples, ``neighbor_ids`` (which dedups) yields
  the same target multiset as the Rust ``neighbors_by_type`` for a single
  predicate — count-with-multiplicity is instead taken via ``degree`` /
  ``neighbor_counts``.
* ``_fts`` replicates the core ``full_text_search`` (whole-word, lowercased
  alphanumeric tokens, boolean AND) for the single-word SPB queries, since the
  Python binding doesn't expose the inverted index. Cached per (graph, label, key,
  word).
"""

import re

from rustychickpeas import Direction

# ---- shared ISO-8601 date helpers (mirror src/props.rs) --------------------


def _days_from_civil(y, m, d):
    """Days since 1970-01-01 (Howard Hinnant), so date math is integer compares."""
    y = y - 1 if m <= 2 else y
    era = (y if y >= 0 else y - 399) // 400
    yoe = y - era * 400
    doy = (153 * (m - 3 if m > 2 else m + 9) + 2) // 5 + d - 1
    doe = yoe * 365 + yoe // 4 - yoe // 100 + doy
    return era * 146097 + doe - 719468


def _parse_ms(s):
    """Epoch-ms of an ISO-8601 ``YYYY-MM-DDTHH:MM:SS.mmm`` (timezone ignored)."""
    if len(s) < 10:
        return 0
    try:
        y, m, d = int(s[0:4]), int(s[5:7]), int(s[8:10])
    except ValueError:
        return 0
    day = _days_from_civil(y, m, d)

    def _f(a, b):
        part = s[a:b]
        return int(part) if len(part) == (b - a) and part.isdigit() else 0

    h, mi, se = _f(11, 13), _f(14, 16), _f(17, 19)
    ms = int(s[20:23]) if len(s) >= 23 and s[19:20] == "." and s[20:23].isdigit() else 0
    return day * 86_400_000 + h * 3_600_000 + mi * 60_000 + se * 1_000 + ms


def _epoch_day(s):
    """Epoch-day number of an ISO date prefix, or None if too short / non-numeric."""
    if len(s) < 10:
        return None
    try:
        return _days_from_civil(int(s[0:4]), int(s[5:7]), int(s[8:10]))
    except ValueError:
        return None


def _ymd(s):
    """(year, month, day) of an ISO date prefix, or None."""
    if len(s) < 10:
        return None
    try:
        return (int(s[0:4]), int(s[5:7]), int(s[8:10]))
    except ValueError:
        return None


# ---- resolution / traversal helpers ----------------------------------------


def _nl(g, label):
    """Nodes carrying ``label``, or [] if the label is absent (the binding raises
    on an unknown label; Rust's ``nodes_with_label`` returns None — degrade to [])."""
    try:
        return g.nodes_with_label(label)
    except ValueError:
        return []


def node_by_uri(uri_map, uri):
    return uri_map.get(uri)


def neighbors_in_set(g, node, direction, pred, node_set):
    return [n for n in g.neighbor_ids(node, direction, [pred]) if n in node_set]


def _uri(g, n):
    lst = _col(g, "uri")
    if lst is not None:
        v = lst[n]
        return v if v else "?"
    return g.prop_str(n, "uri") or "?"


# ---- bulk dense-column reads ------------------------------------------------
# A dense column materialized once as a node-indexed Python list turns N per-node
# FFI ``prop_str`` calls into N list indexes. Cached per (graph, key); falls back
# to ``None`` for a sparse/absent column (callers then use ``prop_str``). Built
# lazily on first use (the untimed result call), so the timed runs read it warm —
# the same way the Rust per-query timings run with their columns/indexes warm.

_COL_CACHE = {}


def _col(g, key):
    ck = (id(g), key)
    cached = _COL_CACHE.get(ck, 0)
    if cached != 0:
        return cached
    try:
        c = g.column(key)
    except Exception:
        c = None
    lst = c.to_pylist() if c is not None else None
    _COL_CACHE[ck] = lst
    return lst


def _str_reader(g, key):
    """Return a function node->str|None for ``key``: a list index when the column
    is dense, else a per-node ``prop_str``. A dense string column reads back as ""
    for an absent value, normalized to None to match the Rust ``prop_str``."""
    lst = _col(g, key)
    if lst is not None:
        return lambda n: (lst[n] or None)
    return lambda n: g.prop_str(n, key)


_LABELSET_CACHE = {}


def _label_set(g, label):
    """``nodes_with_label`` as a cached Python set for O(1) membership — the analog
    of Rust resolving the label bitmap once and reusing it across the timing loop."""
    ck = (id(g), label)
    s = _LABELSET_CACHE.get(ck)
    if s is None:
        s = set(_nl(g, label))
        _LABELSET_CACHE[ck] = s
    return s


# ---- full-text replica (whole-word, lowercased alnum tokens, boolean AND) ---

_SPLIT = re.compile(r"[\W_]+", re.UNICODE)
_FTS_CACHE = {}


def _fts(g, key, word, label="CreativeWork"):
    """Node set of ``label`` whose ``key`` text contains the single token ``word``
    (core ``full_text_search`` semantics). Cached per (graph, label, key, word)."""
    ck = (id(g), label, key, word)
    hit = _FTS_CACHE.get(ck)
    if hit is not None:
        return hit
    token = word.lower()
    matched = set()
    seen = {}
    for n in _nl(g, label):
        v = g.prop_str(n, key)
        if not v:
            continue
        ok = seen.get(v)
        if ok is None:
            ok = token in _SPLIT.split(v.lower())
            seen[v] = ok
        if ok:
            matched.add(n)
    _FTS_CACHE[ck] = matched
    return matched


# ===========================================================================
# basic queries
# ===========================================================================


def q1(g, uri_map, topic_uri):
    """q1 — creative works about/mentioning a thing, newest dateModified first."""
    topic = uri_map.get(topic_uri)
    if topic is None:
        return []
    cworks = _label_set(g, "CreativeWork")
    works = set()
    for pred in ("about", "mentions"):
        works.update(neighbors_in_set(g, topic, Direction.Incoming, pred, cworks))
    dm = _str_reader(g, "dateModified")
    rows = [(w, dm(w)) for w in works]
    rows = [(w, d) for (w, d) in rows if d]
    return _rank_date_desc(rows)


def q2(g, uri_map, cw_uri):
    """q2 — resolve one CreativeWork by uri, only if it carries a title."""
    node = uri_map.get(cw_uri)
    if node is None or not g.has_label(node, "CreativeWork"):
        return []
    return [node] if g.prop_str(node, "title") else []


def q3(g, uri_map, topic_uri, limit):
    """q3 — works tagging (about/mentions) a topic, freshest dateCreated first."""
    topic = uri_map.get(topic_uri)
    if topic is None:
        return []
    cworks = _label_set(g, "CreativeWork")
    dc = _str_reader(g, "dateCreated")
    out, seen = [], set()
    for pred in ("about", "mentions"):
        for w in neighbors_in_set(g, topic, Direction.Incoming, pred, cworks):
            d = dc(w)
            if d and w not in seen:
                seen.add(w)
                out.append((w, d))
    return _top_k_by_key(out, limit)


def q4(g, uri_map, topic_uri, limit):
    """q4 — BlogPosts about/mentioning a topic, freshest dateCreated first."""
    topic = uri_map.get(topic_uri)
    if topic is None:
        return []
    blog = _label_set(g, "BlogPost")
    posts = set()
    for pred in ("about", "mentions"):
        posts.update(neighbors_in_set(g, topic, Direction.Incoming, pred, blog))
    dc = _str_reader(g, "dateCreated")
    rows = [(w, dc(w)) for w in posts]
    rows = [(w, d) for (w, d) in rows if d]
    return _top_k_by_key(rows, limit)


def q5(g, cw_type, audience_uri, start, end):
    """q5 — topics (with a label) tagged by type/audience/window works, by count."""
    works = _nl(g, cw_type or "CreativeWork")
    s_ms, e_ms = _parse_ms(start), _parse_ms(end)
    counts = {}
    for cw in works:
        if audience_uri is not None and not _has_audience(g, cw, audience_uri):
            continue
        dt = g.prop_str(cw, "dateModified")
        if not dt:
            continue
        dt_ms = _parse_ms(dt)
        if not (s_ms < dt_ms < e_ms):
            continue
        topics = set(g.neighbor_ids(cw, Direction.Outgoing, ["about", "mentions"]))
        for t in topics:
            if g.prop_str(t, "label"):
                counts[t] = counts.get(t, 0) + 1
    rows = [(g.prop_str(t, "label") or "?", n) for t, n in counts.items()]
    rows.sort(key=lambda r: (-r[1], r[0]))
    return [[k, n] for k, n in rows]


def q7(g, cw_type, after, before, category_uri, audience_uri):
    """q7 — date-range works carrying title/liveCoverage + category/audience facets."""
    out = []
    for w in _nl(g, cw_type):
        created = g.prop_str(w, "dateCreated")
        if not created or not (after <= created <= before):
            continue
        if not g.prop_str(w, "title"):
            continue
        if g.get_property(w, "liveCoverage") is None:
            continue
        if not _facet(g, w, "category", category_uri):
            continue
        if not _facet(g, w, "audience", audience_uri):
            continue
        out.append(w)
    out.sort()
    return out


def q9(g, uri_map, cw_uri, limit):
    """q9 — works related to a focal one by weighted shared-tag score."""
    focal = uri_map.get(cw_uri)
    if focal is None:
        return []
    fa = set(g.neighbor_ids(focal, Direction.Outgoing, ["about"]))
    fm = set(g.neighbor_ids(focal, Direction.Outgoing, ["mentions"]))
    candidates = set()
    for ent in fa | fm:
        for w in g.neighbor_ids(ent, Direction.Incoming, ["about", "mentions"]):
            if w != focal and g.has_label(w, "CreativeWork"):
                candidates.add(w)
    rows = []
    for o in candidates:
        dt = g.prop_str(o, "dateModified")
        if not dt:
            continue
        a2a = m2a = a2m = m2m = 0
        for e in g.neighbor_ids(o, Direction.Outgoing, ["about"]):
            a2a += e in fa
            m2a += e in fm
        for e in g.neighbor_ids(o, Direction.Outgoing, ["mentions"]):
            a2m += e in fa
            m2m += e in fm
        score = 2.0 * a2a + 1.5 * a2m + 1.0 * m2a + 0.5 * m2m
        if score <= 0.0:
            continue
        rows.append((o, dt, score))
    rows.sort(key=lambda r: (-r[2], _neg_str(r[1])))
    rows = rows[:limit] if limit < len(rows) else rows
    return [[_uri(g, o), int(round(score * 2.0))] for (o, _dt, score) in rows]


# ===========================================================================
# advanced queries
# ===========================================================================


def a1(g, uri_map, pred, thing_uri):
    """a1 — works with a (about|mentions) rel to a thing, newest dateModified first."""
    thing = uri_map.get(thing_uri)
    if thing is None:
        return []
    cworks = _label_set(g, "CreativeWork")
    dm = _str_reader(g, "dateModified")
    rows = []
    for w in neighbors_in_set(g, thing, Direction.Incoming, pred, cworks):
        d = dm(w)
        if d:
            rows.append((w, d))
    return _rank_date_desc(rows)


def a2(g, uri_map, cw_uri):
    """a2 — the CreativeWork subtype local names of one titled work, sorted."""
    work = uri_map.get(cw_uri)
    if work is None or not g.has_label(work, "CreativeWork"):
        return []
    if not g.prop_str(work, "title"):
        return []
    subs = [s for s in ("BlogPost", "NewsItem", "Programme") if g.has_label(work, s)]
    subs.sort()
    return subs


def a3(g, after, before):
    """a3 — in-window works grouped by minute-of-hour of dateModified, by count."""
    counts = {}
    for w in _nl(g, "CreativeWork"):
        dt = g.prop_str(w, "dateModified")
        if not dt or not (after < dt < before):
            continue
        mm = dt[14:16]
        if len(mm) == 2 and mm.isdigit():
            minute = int(mm)
            counts[minute] = counts.get(minute, 0) + 1
    rows = sorted(counts.items(), key=lambda r: (-r[1], r[0]))
    return [[str(m), n] for m, n in rows]


def a4(g, after, before, limit):
    """a4 — CreativeWork subtypes ranked by in-window dateModified count."""
    rows = []
    for label in ("BlogPost", "NewsItem", "Programme"):
        n = 0
        for w in _nl(g, label):
            dm = g.prop_str(w, "dateModified")
            if dm and after < dm < before:
                n += 1
        if n > 0:
            rows.append((label, n))
    rows.sort(key=lambda r: (-r[1], r[0]))
    rows = rows[:limit] if limit < len(rows) else rows
    return [[k, n] for k, n in rows]


def a5(g, entity_label, cat1, cat2, limit):
    """a5 — about-things of an entity type ranked by works in either category."""
    entities = _label_set(g, entity_label)
    counts = {}
    for w in _nl(g, "CreativeWork"):
        if not _in_either_category(g, w, cat1, cat2):
            continue
        for about in g.neighbor_ids(w, Direction.Outgoing, ["about"]):
            if about in entities:
                counts[about] = counts.get(about, 0) + 1
    ranked = _top_k_pairs(counts, limit)  # (node, count): count desc, node asc
    return [[_uri(g, a), n] for a, n in ranked]


def a6(g, live_coverage, audience_uri, limit):
    """a6 — about-entity types ranked over live/audience works (incl. Thing)."""
    type_sets = []
    for ty in ("Company", "Event", "Thing"):
        s = _label_set(g, ty)
        if s:
            type_sets.append((ty, s))
    counts = {}
    for w in _nl(g, "CreativeWork"):
        lc = g.get_property(w, "liveCoverage")
        if (lc if lc is not None else False) != live_coverage:
            continue
        if not _has_audience(g, w, audience_uri):
            continue
        for about in g.neighbor_ids(w, Direction.Outgoing, ["about"]):
            for ty, s in type_sets:
                if about in s:
                    counts[ty] = counts.get(ty, 0) + 1
    rows = sorted(counts.items(), key=lambda r: (-r[1], r[0]))
    rows = rows[:limit] if limit < len(rows) else rows
    return [[k, n] for k, n in rows]


def a7(g, min_primary_content, limit):
    """a7 — mention targets ranked over works with >threshold primaryContentOf."""
    qualifying = [
        w
        for w in _nl(g, "CreativeWork")
        if g.degree(w, Direction.Outgoing, "primaryContentOf") > min_primary_content
    ]
    counts = g.neighbor_counts(qualifying, Direction.Outgoing, "mentions")
    ranked = _top_k_pairs(counts, limit)
    return [[_uri(g, m), n] for m, n in ranked]


def a8(g, cw_type, audience_uri, after, before):
    """a8 — topics (tag = materialized about/mentions) ranked by type/audience/window."""
    qualifying = []
    for w in _nl(g, cw_type):
        dt = g.prop_str(w, "dateModified")
        if dt and after < dt < before and _has_audience(g, w, audience_uri):
            qualifying.append(w)
    counts = g.neighbor_counts(qualifying, Direction.Outgoing, "tag")
    ranked = _top_k_pairs(counts, 1 << 62)
    return [[_uri(g, t), n] for t, n in ranked]


def a9(g):
    """a9 — the largest outgoing ``mentions`` count on any CreativeWork."""
    best = 0
    for w in _nl(g, "CreativeWork"):
        c = g.degree(w, Direction.Outgoing, "mentions")
        if c > best:
            best = c
    return best


def a10(g, limit):
    """a10 — works whose mentions count equals the max and that carry dateCreated."""
    best = 0
    counts = []
    for w in _nl(g, "CreativeWork"):
        c = g.degree(w, Direction.Outgoing, "mentions")
        counts.append((w, c))
        if c > best:
            best = c
    if best == 0:
        return []
    rows = [
        (_uri(g, w), c)
        for (w, c) in counts
        if c == best and g.prop_str(w, "dateCreated")
    ]
    rows.sort(key=lambda r: r[0])
    rows = rows[:limit] if limit < len(rows) else rows
    return [[u, c] for u, c in rows]


def a13(g, cat1, cat2, limit):
    """a13 — (work, tag) pairs for works in either category with a dateModified."""
    pairs = []
    for w in _nl(g, "CreativeWork"):
        if not _in_either_category(g, w, cat1, cat2):
            continue
        if not g.prop_str(w, "dateModified"):
            continue
        for tag in g.neighbor_ids(w, Direction.Outgoing, ["tag"]):
            pairs.append((w, tag))
    pairs = sorted(set(pairs))
    pairs = pairs[:limit] if limit < len(pairs) else pairs
    uris = _col(g, "uri")
    if uris is not None:
        return [[uris[w] or "?", uris[tag]] for (w, tag) in pairs if uris[tag]]
    out = []
    for w, tag in pairs:
        tu = g.prop_str(tag, "uri")
        if tu is not None:
            out.append([_uri(g, w), tu])
    return out


def a14(g, uri_map, primary_format_uri, web_doc_type, limit):
    """a14 — full star works pinned to a primaryFormat + web-document type, newest first."""
    pf = uri_map.get(primary_format_uri)
    wdt = uri_map.get(web_doc_type)
    if pf is None or wdt is None:
        return []
    rows = []
    for w in _nl(g, "CreativeWork"):
        if not (
            g.has_rel(w, Direction.Outgoing, "tag")
            and g.has_rel(w, Direction.Outgoing, "category")
            and g.has_rel(w, Direction.Outgoing, "thumbnail")
            and g.has_rel(w, Direction.Outgoing, "audience")
        ):
            continue
        if pf not in g.neighbor_ids(w, Direction.Outgoing, ["primaryFormat"]):
            continue
        if not _has_web_doc(g, w, wdt):
            continue
        d = g.prop_str(w, "dateModified")
        if d:
            rows.append((w, d))
    return _top_k_by_key(rows, limit)


def a15(g, word, limit):
    """a15 — title-FTS works with a category and an about/mentions sharing Thing."""
    out = []
    for w in _fts(g, "title", word):
        if not g.neighbor_ids(w, Direction.Outgoing, ["category"]):
            continue
        if _about_and_mentions_share_thing(g, w):
            out.append(w)
    out.sort()
    out = out[:limit] if limit < len(out) else out
    return out


def a16(g, word, limit):
    """a16 — (work, tag) pairs for title-FTS works with a category, ordered by tag."""
    uris = _col(g, "uri")
    rows = set()
    for w in _fts(g, "title", word):
        if not g.prop_str(w, "title"):
            continue
        if not g.neighbor_ids(w, Direction.Outgoing, ["category"]):
            continue
        wu = uris[w] if uris is not None else g.prop_str(w, "uri")
        if not wu:
            continue
        for tag in g.neighbor_ids(w, Direction.Outgoing, ["tag"]):
            tu = uris[tag] if uris is not None else g.prop_str(tag, "uri")
            if tu:
                rows.add((tu, wu))
    ordered = sorted(rows)
    ordered = ordered[:limit] if limit < len(ordered) else ordered
    return [[work, tag] for (tag, work) in ordered]


def a17(g, lat, lon, deviation):
    """a17 — works mentioning a Feature in the lat/long box, carrying dateModified."""
    lo_la, hi_la = lat - deviation, lat + deviation
    lo_lo, hi_lo = lon - deviation, lon + deviation
    cworks = _label_set(g, "CreativeWork")
    works = set()
    for f in _nl(g, "Feature"):
        la = g.get_property(f, "lat")
        if la is None or not (lo_la <= la <= hi_la):
            continue
        lo = g.get_property(f, "long")
        if lo is None or not (lo_lo <= lo <= hi_lo):
            continue
        for w in neighbors_in_set(g, f, Direction.Incoming, "mentions", cworks):
            if g.prop_str(w, "dateModified"):
                works.add(w)
    return sorted(works)


def a18(g, cw_type, after, before, limit):
    """a18 — in-range works carrying title/liveCoverage + category/audience, newest first."""
    rows = []
    for w in _nl(g, cw_type):
        modified = g.prop_str(w, "dateModified")
        if not modified or not (after <= modified <= before):
            continue
        if not g.prop_str(w, "title"):
            continue
        if g.get_property(w, "liveCoverage") is None:
            continue
        if not g.has_rel(w, Direction.Outgoing, "category"):
            continue
        if not g.has_rel(w, Direction.Outgoing, "audience"):
            continue
        rows.append((w, modified))
    return _top_k_by_key(rows, limit)


def a19(g, cw_type, audience_uri, start, end, limit):
    """a19 — topics by newest tagging-work modification then count (label or uri)."""
    s_ms, e_ms = _parse_ms(start), _parse_ms(end)
    acc = {}  # topic -> [count, max_ms, max_date_str]
    for cw in _nl(g, "CreativeWork"):
        if cw_type is not None and not g.has_label(cw, cw_type):
            continue
        if audience_uri is not None and not _has_audience(g, cw, audience_uri):
            continue
        dt = g.prop_str(cw, "dateModified")
        if not dt:
            continue
        dt_ms = _parse_ms(dt)
        if dt_ms < s_ms or dt_ms > e_ms:
            continue
        topics = set(g.neighbor_ids(cw, Direction.Outgoing, ["about", "mentions", "tag"]))
        for t in topics:
            e = acc.get(t)
            if e is None:
                acc[t] = [1, dt_ms, dt]
            else:
                e[0] += 1
                if dt_ms > e[1]:
                    e[1] = dt_ms
                    e[2] = dt
    rows = [(t, c, ms, date) for t, (c, ms, date) in acc.items()]
    rows.sort(key=lambda r: (-r[2], -r[1], r[0]))
    rows = rows[:limit] if limit < len(rows) else rows
    out = []
    for t, c, _ms, date in rows:
        name = g.prop_str(t, "label") or g.prop_str(t, "uri") or "?"
        out.append([name, c, date])
    return out


def a20(g, word, limit):
    """a20 — works whose title OR description contains the word, newest first."""
    hits = _fts(g, "description", word) | _fts(g, "title", word)
    rows = []
    for w in hits:
        d = g.prop_str(w, "dateModified")
        if d:
            rows.append((w, d))
    return _top_k_by_key(rows, limit)


def a21(g, word, category_uri, audience_uri, tag_uri, live, date_from, date_to, limit):
    """a21 — title-FTS faceted search (category/audience/tag/liveCoverage/date)."""
    out = []
    for w in _fts(g, "title", word):
        if _a21_facets(g, w, category_uri, audience_uri, tag_uri, live, date_from, date_to):
            out.append(w)
    out.sort()
    out = out[:limit] if limit < len(out) else out
    return out


def a22(g, word, category_uri, audience_uri, tag_uri, after, before, live, limit):
    """a22 — title-FTS faceted search with full BGP + a dateCreated range facet."""
    out = []
    for w in _fts(g, "title", word):
        created = g.prop_str(w, "dateCreated")
        if not created:
            continue
        if not g.prop_str(w, "description"):
            continue
        if g.get_property(w, "liveCoverage") is None:
            continue
        if not g.has_rel(w, Direction.Outgoing, "primaryFormat"):
            continue
        if not _facet(g, w, "category", category_uri):
            continue
        if not _facet(g, w, "audience", audience_uri):
            continue
        if not _folded_tag(g, w, tag_uri):
            continue
        if after is not None and created < after:
            continue
        if before is not None and created > before:
            continue
        if live is not None:
            lc = g.get_property(w, "liveCoverage")
            if (lc if lc is not None else False) != live:
                continue
        out.append(w)
    out.sort()
    out = out[:limit] if limit < len(out) else out
    return out


def a23(g, word, category_uri, limit):
    """a23 — per topic, the distinct dateCreated days over title-FTS+category works."""
    by_tag = {}
    for w in _fts(g, "title", word):
        if not g.has_neighbor_with_property(
            w, Direction.Outgoing, "category", "uri", category_uri
        ):
            continue
        created = g.prop_str(w, "dateCreated")
        if not created:
            continue
        if (
            not g.prop_str(w, "description")
            or not g.has_rel(w, Direction.Outgoing, "audience")
            or not g.has_rel(w, Direction.Outgoing, "primaryFormat")
            or g.get_property(w, "liveCoverage") is None
        ):
            continue
        day = _epoch_day(created)
        if day is None:
            continue
        for t in g.neighbor_ids(w, Direction.Outgoing, ["about", "mentions"]):
            by_tag.setdefault(t, set()).add(day)
    rows = [(_uri(g, t), len(days)) for t, days in by_tag.items()]
    rows.sort(key=lambda r: (-r[1], r[0]))
    rows = rows[:limit] if limit < len(rows) else rows
    return [[u, n] for u, n in rows]


def a24(g, uri_map, uri_a, uri_b, date_from=None, date_to=None):
    """a24 — per-day count of works tagging (about) BOTH entities, ascending."""
    a = uri_map.get(uri_a)
    b = uri_map.get(uri_b)
    if a is None or b is None:
        return []
    about_a = set(g.neighbor_ids(a, Direction.Incoming, ["about"]))
    both = set()
    for w in g.neighbor_ids(b, Direction.Incoming, ["about"]):
        if w in about_a and g.has_label(w, "CreativeWork"):
            both.add(w)
    per_day = {}
    for w in both:
        created = g.prop_str(w, "dateCreated")
        if not created:
            continue
        key = _ymd(created)
        if key is None:
            continue
        day = created[:10]
        if (date_from is not None and day < date_from) or (
            date_to is not None and day > date_to
        ):
            continue
        per_day[key] = per_day.get(key, 0) + 1
    rows = sorted(per_day.items())
    return [["%04d-%02d-%02d" % k, n] for k, n in rows]


def a25(g, uri_map, uri_a, limit):
    """a25 — entities co-occurring with A by distinct co-mention days, ranked."""
    a = uri_map.get(uri_a)
    if a is None:
        return []
    days = {}
    for cw in g.neighbor_ids(a, Direction.Incoming, ["about"]):
        if not g.has_label(cw, "CreativeWork"):
            continue
        created = g.prop_str(cw, "dateCreated")
        if not created or len(created) < 10:
            continue
        day = created[:10]
        for who in g.neighbor_ids(cw, Direction.Outgoing, ["about"]):
            if who != a:
                days.setdefault(who, set()).add(day)
    rows = [(who, len(s)) for who, s in days.items()]
    rows.sort(key=lambda r: (-r[1], r[0]))  # days desc, node id asc (uri-order proxy)
    rows = rows[:limit] if limit < len(rows) else rows
    return [[_uri(g, who), n] for who, n in rows]


# ---- small shared predicates ----------------------------------------------


def _neg_str(s):
    """Sort key that orders strings descending under an ascending sort."""
    return _Desc(s)


class _Desc:
    """Wrap a string so ascending tuple sorts place it in descending order."""

    __slots__ = ("s",)

    def __init__(self, s):
        self.s = s

    def __lt__(self, other):
        return self.s > other.s

    def __eq__(self, other):
        return self.s == other.s


def _rank_date_desc(rows):
    """(node, date_str) -> nodes ordered date desc, node id asc. Two stable passes
    (id asc, then date desc) avoid allocating a reverse-key wrapper per row."""
    rows.sort(key=lambda r: r[0])
    rows.sort(key=lambda r: r[1], reverse=True)
    return [w for (w, _) in rows]


def _top_k_by_key(rows, limit):
    """(node, key) -> nodes ranked key desc, node id asc, truncated to limit."""
    rows = list(rows)
    rows.sort(key=lambda r: r[0])
    rows.sort(key=lambda r: r[1], reverse=True)
    if limit < len(rows):
        rows = rows[:limit]
    return [w for (w, _) in rows]


def _top_k_pairs(counts, limit):
    """{node: count} -> [(node, count)] ranked count desc, node id asc, truncated."""
    rows = sorted(counts.items(), key=lambda r: (-r[1], r[0]))
    if limit < len(rows):
        rows = rows[:limit]
    return rows


def _has_audience(g, w, audience_uri):
    return g.has_neighbor_with_property(w, Direction.Outgoing, "audience", "uri", audience_uri)


def _facet(g, w, rel, want_uri):
    if want_uri is None:
        return g.has_rel(w, Direction.Outgoing, rel)
    return g.has_neighbor_with_property(w, Direction.Outgoing, rel, "uri", want_uri)


def _folded_tag(g, w, want_uri):
    return _facet(g, w, "about", want_uri) or _facet(g, w, "mentions", want_uri)


def _in_either_category(g, w, cat1, cat2):
    uris = _col(g, "uri")
    if uris is not None:
        for c in g.neighbor_ids(w, Direction.Outgoing, ["category"]):
            u = uris[c]
            if u == cat1 or u == cat2:
                return True
        return False
    for c in g.neighbor_ids(w, Direction.Outgoing, ["category"]):
        u = g.prop_str(c, "uri")
        if u == cat1 or u == cat2:
            return True
    return False


def _has_web_doc(g, w, wdt):
    for pc in g.neighbor_ids(w, Direction.Outgoing, ["primaryContentOf"]):
        if wdt in g.neighbor_ids(pc, Direction.Outgoing, ["webDocumentType"]):
            return True
    return False


def _about_and_mentions_share_thing(g, w):
    about_thing = any(
        g.has_label(a, "Thing")
        for a in g.neighbor_ids(w, Direction.Outgoing, ["about"])
    )
    if not about_thing:
        return False
    return any(
        g.has_label(m, "Thing")
        for m in g.neighbor_ids(w, Direction.Outgoing, ["mentions"])
    )


def _a21_facets(g, w, category_uri, audience_uri, tag_uri, live, date_from, date_to):
    if not g.neighbor_ids(w, Direction.Outgoing, ["about"]) and not g.neighbor_ids(
        w, Direction.Outgoing, ["mentions"]
    ):
        return False
    if category_uri is not None and not _facet(g, w, "category", category_uri):
        return False
    if audience_uri is not None and not _facet(g, w, "audience", audience_uri):
        return False
    if tag_uri is not None and not _folded_tag(g, w, tag_uri):
        return False
    if live is not None:
        lc = g.get_property(w, "liveCoverage")
        if (lc if lc is not None else False) != live:
            return False
    if date_from is not None or date_to is not None:
        created = g.prop_str(w, "dateCreated")
        if created is None:
            return False
        if date_from is not None and created < date_from:
            return False
        if date_to is not None and created > date_to:
            return False
    return True

"""Load LDBC SNB BI data into a rustychickpeas ``GraphSnapshot`` from Python.

Strategy (fast *for Python*): read the partitioned, gzipped, pipe-delimited
LDBC CSVs once, derive the properties the queries need, and write a normalized
comma-CSV per label that the native bulk loader ingests in one call. The derive
pass is plain stdlib (csv + gzip); the heavy node build happens in Rust.

Each node is given an auto-assigned internal id; the external LDBC id is stored
as the ``id`` property so relationships can be resolved by property reference
(see ``GraphSnapshotBuilder.load_relationships_from_csv`` with a dict endpoint).
"""

import csv
import glob
import gzip
import os
import tempfile

from rustychickpeas import GraphSnapshotBuilder, Ref, Rel, Prop

import props


def iter_rows(entity_dir: str, cols):
    """Yield the requested columns (in order) for every row across all
    ``part-*.csv.gz`` files in ``entity_dir``."""
    files = sorted(glob.glob(os.path.join(entity_dir, "part-*.csv.gz")))
    for path in files:
        with gzip.open(path, "rt", newline="", encoding="utf-8") as fh:
            reader = csv.reader(fh, delimiter="|")
            header = next(reader)
            idx = [header.index(c) for c in cols]
            for row in reader:
                yield [row[i] for i in idx]


def _normalize_messages(entity_dir: str, out_path: str, with_lang: bool = False) -> int:
    """Write ``id,year,day,len,content`` (+ ``lang`` for Post) for one message
    label, deriving year/day from creationDate. ``content`` is stored as 0/1.
    Returns the row count.
    """
    src = ["id", "creationDate", "content", "length"] + (["language"] if with_lang else [])
    header = ["id", "year", "day", "len", "content"] + (["lang"] if with_lang else [])
    n = 0
    with open(out_path, "w", newline="", encoding="utf-8") as out:
        w = csv.writer(out)
        w.writerow(header)
        date_cache = {}
        for row in iter_rows(entity_dir, src):
            ext_id, cdate, content, length = row[0], row[1], row[2], row[3]
            # Many messages share a calendar day; memoize the day math on the
            # YYYY-MM-DD prefix (~1-2k distinct days vs millions of rows).
            prefix = cdate[:10]
            yd = date_cache.get(prefix)
            if yd is None:
                parsed = props.parse_date(cdate)
                yd = parsed if parsed is not None else (0, 0)
                date_cache[prefix] = yd
            year, day = yd
            ln = int(length) if length else 0
            out_row = [ext_id, year, day, ln, 1 if content else 0]
            if with_lang:
                out_row.append(row[4])
            w.writerow(out_row)
            n += 1
    return n


def _normalize_forums(entity_dir: str, out_path: str) -> int:
    """Write ``id,title,fday`` for forums, deriving fday (creation day) from
    creationDate so BI Q4's forum-age filter has it. Returns the row count."""
    n = 0
    with open(out_path, "w", newline="", encoding="utf-8") as out:
        w = csv.writer(out)
        w.writerow(["id", "title", "fday"])
        date_cache = {}
        for ext_id, title, cdate in iter_rows(entity_dir, ["id", "title", "creationDate"]):
            prefix = cdate[:10]
            day = date_cache.get(prefix)
            if day is None:
                parsed = props.parse_date(cdate)
                day = parsed[1] if parsed is not None else 0
                date_cache[prefix] = day
            w.writerow([ext_id, title, day])
            n += 1
    return n


def _normalize_knows(entity_dir: str, out_path: str) -> int:
    """Write ``Person1Id,Person2Id,kd`` for knows edges, deriving kd (creation day)
    from creationDate so BI Q11 can date-filter knows edges. Returns the row count."""
    n = 0
    with open(out_path, "w", newline="", encoding="utf-8") as out:
        w = csv.writer(out)
        w.writerow(["Person1Id", "Person2Id", "kd"])
        date_cache = {}
        for cdate, p1, p2 in iter_rows(entity_dir, ["creationDate", "Person1Id", "Person2Id"]):
            prefix = cdate[:10]
            day = date_cache.get(prefix)
            if day is None:
                parsed = props.parse_date(cdate)
                day = parsed[1] if parsed is not None else 0
                date_cache[prefix] = day
            w.writerow([p1, p2, day])
            n += 1
    return n


def load_messages(snapshot_path: str):
    """Build a snapshot containing Post + Comment message nodes (BI Q1's inputs).

    Returns ``(snapshot, stats)`` where ``stats`` is ``{"posts": .., "comments": ..}``.
    """
    dynamic = os.path.join(snapshot_path, "dynamic")
    builder = GraphSnapshotBuilder(capacity_nodes=4_000_000)
    props_cols = ["id", "year", "day", "len", "content"]
    stats = {}

    with tempfile.TemporaryDirectory() as tmp:
        for label, entity in [("Post", "Post"), ("Comment", "Comment")]:
            norm = os.path.join(tmp, f"{entity}.csv")
            count = _normalize_messages(os.path.join(dynamic, entity), norm)
            builder.load_nodes_from_csv(
                norm,
                property_columns=props_cols,
                default_label=label,
            )
            stats[entity.lower() + "s"] = count

    return builder.finalize(), stats


def _load_nodes(builder, entity_dir: str, **kwargs) -> int:
    """Load every ``part-*.csv.gz`` in ``entity_dir`` as nodes (pipe-delimited,
    straight off the raw files). Returns the node count."""
    n = 0
    for path in sorted(glob.glob(os.path.join(entity_dir, "part-*.csv.gz"))):
        n += len(builder.load_nodes_from_csv(path, delimiter="|", **kwargs))
    return n


def _load_edges(builder, entity_dir: str, start_ref, end_ref, rel_type: str) -> int:
    """Load every ``part-*.csv.gz`` in ``entity_dir`` as relationships, resolving
    endpoints by the property refs (empty FK cells are skipped). Returns the count."""
    n = 0
    for path in sorted(glob.glob(os.path.join(entity_dir, "part-*.csv.gz"))):
        n += len(
            builder.load_relationships_from_csv(
                path, start_ref, end_ref, fixed_rel_type=rel_type, delimiter="|"
            )
        )
    return n


def _ref(column: str, label: str) -> dict:
    """A property-ref endpoint: resolve the FK ``column`` to a node of ``label``
    by its ``id`` property."""
    return {"column": column, "property_key": "id", "label": label}


def _load_rels_multi(builder, entity_dir: str, rels) -> int:
    """Load several relationship types from each ``part-*.csv.gz`` in ``entity_dir``
    in one pass per part (one read, shared node indexes). Returns the total count."""
    n = 0
    for path in sorted(glob.glob(os.path.join(entity_dir, "part-*.csv.gz"))):
        n += sum(builder.load_relationships_from_csv_multi(path, rels, delimiter="|"))
    return n


def load_bi_graph(snapshot_path: str):
    """Build the full LDBC SNB BI graph (mirrors src/loader.rs): all entities and
    edges, so query timings are comparable to the Rust bench. Nodes are loaded in
    the same order as the Rust loader (so internal ids align) and carry their
    external LDBC id as the ``id`` property; edges resolve by that property.

    Node props beyond what the ported queries read are kept minimal (id + name);
    edge properties (join dates, weights) are not loaded yet. Returns
    ``(snapshot, stats)``.
    """
    dynamic = os.path.join(snapshot_path, "dynamic")
    static = os.path.join(snapshot_path, "static")
    b = GraphSnapshotBuilder(capacity_nodes=4_000_000, capacity_rels=24_000_000)
    s = {}

    with tempfile.TemporaryDirectory() as tmp:
        # --- NODES, in src/loader.rs order so internal ids align ---
        s["tagclasses"] = _load_nodes(b, f"{static}/TagClass", property_columns=["id", "name"], default_label="TagClass")
        s["tags"] = _load_nodes(b, f"{static}/Tag", property_columns=["id", "name"], default_label="Tag")
        s["persons"] = _load_nodes(b, f"{dynamic}/Person", property_columns=["id"], default_label="Person")
        # Place/Organisation get a super-label so id-refs that span their subtypes
        # (City/Country/Continent, Company/University) resolve.
        s["places"] = _load_nodes(b, f"{static}/Place", property_columns=["id", "name"], label_columns=["type"], default_label="Place")
        forum_csv = os.path.join(tmp, "Forum.csv")
        s["forums"] = _normalize_forums(f"{dynamic}/Forum", forum_csv)
        b.load_nodes_from_csv(forum_csv, property_columns=["id", "title", "fday"], default_label="Forum")

        post_csv = os.path.join(tmp, "Post.csv")
        s["posts"] = _normalize_messages(f"{dynamic}/Post", post_csv, with_lang=True)
        b.load_nodes_from_csv(post_csv, property_columns=["id", "year", "day", "len", "content", "lang"], default_label="Post")
        comment_csv = os.path.join(tmp, "Comment.csv")
        s["comments"] = _normalize_messages(f"{dynamic}/Comment", comment_csv)
        b.load_nodes_from_csv(comment_csv, property_columns=["id", "year", "day", "len", "content"], default_label="Comment")

        s["orgs"] = _load_nodes(b, f"{static}/Organisation", property_columns=["id", "name"], label_columns=["type"], default_label="Organisation")

        # --- EDGES (resolved by the "id" property; loaded off the raw gz) ---
        # Single-rel-per-file edges.
        edges = [
            (f"{static}/TagClass", _ref("id", "TagClass"), _ref("SubclassOfTagClassId", "TagClass"), "isSubclassOf"),
            (f"{static}/Tag", _ref("id", "Tag"), _ref("TypeTagClassId", "TagClass"), "hasType"),
            (f"{static}/Place", _ref("id", "Place"), _ref("PartOfPlaceId", "Place"), "isPartOf"),
            (f"{dynamic}/Person", _ref("id", "Person"), _ref("LocationCityId", "City"), "isLocatedIn"),
            (f"{dynamic}/Forum", _ref("id", "Forum"), _ref("ModeratorPersonId", "Person"), "hasModerator"),
            (f"{dynamic}/Forum_hasMember_Person", _ref("ForumId", "Forum"), _ref("PersonId", "Person"), "hasMember"),
            (f"{dynamic}/Post_hasTag_Tag", _ref("PostId", "Post"), _ref("TagId", "Tag"), "hasTag"),
            (f"{dynamic}/Comment_hasTag_Tag", _ref("CommentId", "Comment"), _ref("TagId", "Tag"), "hasTag"),
            (f"{dynamic}/Person_hasInterest_Tag", _ref("personId", "Person"), _ref("interestId", "Tag"), "hasInterest"),
            (f"{dynamic}/Person_likes_Post", _ref("PersonId", "Person"), _ref("PostId", "Post"), "likes"),
            (f"{dynamic}/Person_likes_Comment", _ref("PersonId", "Person"), _ref("CommentId", "Comment"), "likes"),
            # knows is loaded below with its creationDate (kd) edge property.
            (f"{static}/Organisation", _ref("id", "Organisation"), _ref("LocationPlaceId", "Place"), "orgPlace"),
            (f"{dynamic}/Person_workAt_Company", _ref("PersonId", "Person"), _ref("CompanyId", "Company"), "workAt"),
            (f"{dynamic}/Person_studyAt_University", _ref("PersonId", "Person"), _ref("UniversityId", "University"), "studyAt"),
        ]
        total = 0
        for entity_dir, start_ref, end_ref, rel_type in edges:
            total += _load_edges(b, entity_dir, start_ref, end_ref, rel_type)

        # Merged-FK message files: several rels per file, one pass each.
        total += _load_rels_multi(b, f"{dynamic}/Post", [
            Rel("hasCreator", Ref("CreatorPersonId", "Person"), Ref("id", "Post")),
            Rel("containerOf", Ref("ContainerForumId", "Forum"), Ref("id", "Post")),
            Rel("msgCountry", Ref("id", "Post"), Ref("LocationCountryId", "Country")),
        ])
        total += _load_rels_multi(b, f"{dynamic}/Comment", [
            Rel("hasCreator", Ref("CreatorPersonId", "Person"), Ref("id", "Comment")),
            Rel("msgCountry", Ref("id", "Comment"), Ref("LocationCountryId", "Country")),
            # replyOf parent is a Post or a Comment — both refs in one pass; empty cells skip.
            Rel("replyOf", Ref("id", "Comment"), Ref("ParentPostId", "Post")),
            Rel("replyOf", Ref("id", "Comment"), Ref("ParentCommentId", "Comment")),
        ])

        # knows (undirected -> both directions) carrying its creationDate as the kd
        # edge property (epoch day), so Q11 can date-filter knows edges.
        knows_csv = os.path.join(tmp, "knows.csv")
        _normalize_knows(f"{dynamic}/Person_knows_Person", knows_csv)
        total += sum(b.load_relationships_from_csv_multi(knows_csv, [
            Rel("knows", Ref("Person1Id", "Person"), Ref("Person2Id", "Person"), props=[Prop("kd", "kd", int)]),
            Rel("knows", Ref("Person2Id", "Person"), Ref("Person1Id", "Person"), props=[Prop("kd", "kd", int)]),
        ]))
        s["edges"] = total

    return b.finalize(), s

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

from rustychickpeas import GraphSnapshotBuilder

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


def _normalize_messages(entity_dir: str, out_path: str) -> int:
    """Write ``id,year,day,len,content`` for one message label (Post/Comment).

    ``content`` is stored as 0/1 (the queries only need "has content", and the
    Python column reader exports dense i64 columns). Returns the row count.
    """
    n = 0
    with open(out_path, "w", newline="", encoding="utf-8") as out:
        w = csv.writer(out)
        w.writerow(["id", "year", "day", "len", "content"])
        for ext_id, cdate, content, length in iter_rows(
            entity_dir, ["id", "creationDate", "content", "length"]
        ):
            parsed = props.parse_date(cdate)
            year, day = parsed if parsed is not None else (0, 0)
            ln = int(length) if length else 0
            has_content = 1 if content else 0
            w.writerow([ext_id, year, day, ln, has_content])
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


def _write_csv(entity_dir: str, out_path: str, cols) -> int:
    """Concatenate ``cols`` (in order) from a partitioned LDBC entity dir into one
    comma-CSV (header = ``cols``). Returns the row count."""
    n = 0
    with open(out_path, "w", newline="", encoding="utf-8") as out:
        w = csv.writer(out)
        w.writerow(cols)
        for row in iter_rows(entity_dir, cols):
            w.writerow(row)
            n += 1
    return n


def load_bi_graph(snapshot_path: str):
    """Build the BI subgraph used by Q1/Q2: Post/Comment message nodes, Tag and
    TagClass nodes, and the ``hasType`` (Tag->TagClass) and ``hasTag``
    (Message->Tag) edges. Nodes carry their external LDBC id as the ``id``
    property; edges are resolved by that property (see the property-ref CSV
    loader). Returns ``(snapshot, stats)``.
    """
    dynamic = os.path.join(snapshot_path, "dynamic")
    static = os.path.join(snapshot_path, "static")
    builder = GraphSnapshotBuilder(capacity_nodes=4_000_000, capacity_rels=16_000_000)
    stats = {}

    with tempfile.TemporaryDirectory() as tmp:
        # --- nodes (all before any edges, so property refs resolve) ---
        for label in ("Post", "Comment"):
            norm = os.path.join(tmp, f"{label}.csv")
            stats[label.lower() + "s"] = _normalize_messages(os.path.join(dynamic, label), norm)
            builder.load_nodes_from_csv(
                norm, property_columns=["id", "year", "day", "len", "content"], default_label=label
            )

        tagclass_csv = os.path.join(tmp, "TagClass.csv")
        stats["tagclasses"] = _write_csv(os.path.join(static, "TagClass"), tagclass_csv, ["id", "name"])
        builder.load_nodes_from_csv(
            tagclass_csv, property_columns=["id", "name"], default_label="TagClass"
        )

        # Tag file keeps TypeTagClassId for the hasType edge below.
        tag_csv = os.path.join(tmp, "Tag.csv")
        stats["tags"] = _write_csv(
            os.path.join(static, "Tag"), tag_csv, ["id", "name", "TypeTagClassId"]
        )
        builder.load_nodes_from_csv(tag_csv, property_columns=["id", "name"], default_label="Tag")

        # --- edges (resolved by the "id" property) ---
        builder.load_relationships_from_csv(
            tag_csv,
            {"column": "id", "property_key": "id", "label": "Tag"},
            {"column": "TypeTagClassId", "property_key": "id", "label": "TagClass"},
            fixed_rel_type="hasType",
        )

        hastag = 0
        for entity, id_col, src in (
            ("Post_hasTag_Tag", "PostId", "Post"),
            ("Comment_hasTag_Tag", "CommentId", "Comment"),
        ):
            edge_csv = os.path.join(tmp, f"{entity}.csv")
            _write_csv(os.path.join(dynamic, entity), edge_csv, [id_col, "TagId"])
            pairs = builder.load_relationships_from_csv(
                edge_csv,
                {"column": id_col, "property_key": "id", "label": src},
                {"column": "TagId", "property_key": "id", "label": "Tag"},
                fixed_rel_type="hasTag",
            )
            hastag += len(pairs)
        stats["hastag_edges"] = hastag

    return builder.finalize(), stats

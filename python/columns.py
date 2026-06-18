"""Fast dense-column access for the Python BI queries.

``arrow(g, key)`` returns a dense column as a zero-copy pyarrow Array built over
``GraphSnapshot.column(key)`` (a self-describing, buffer-protocol object) — no
per-element Python ints, the basis for vectorized column scans. Returns ``None``
when the column is absent or not stored densely.

dtype mapping: int64 -> int64, float64 -> double, bool -> uint8 (0/1),
string -> uint32 (interned codes; resolve filter targets with ``g.string_id``).
"""

import pyarrow as pa

_ARROW_TYPE = {
    "int64": pa.int64(),
    "float64": pa.float64(),
    "bool": pa.uint8(),
    "string": pa.uint32(),
}


def arrow(g, key):
    col = g.column(key)
    if col is None:
        return None
    # py_buffer wraps the column's buffer zero-copy; from_buffers with no null bitmap.
    return pa.Array.from_buffers(_ARROW_TYPE[col.dtype], len(col), [None, pa.py_buffer(col)])

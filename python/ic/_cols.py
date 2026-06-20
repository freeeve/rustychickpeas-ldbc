"""Dense i64 column reader for the IC queries — an O(1) per-node reader via the
column's buffer protocol (a ``memoryview`` indexed by node id, C-speed, no per-node
call), falling back to ``get_property`` when the column isn't densely bufferable
(tiny/sparse graphs). No pyarrow/numpy — just our native column buffer.
"""


def i64_reader(g, key):
    col = g.column(key)
    if col is not None:
        try:
            mv = memoryview(col)
            return lambda n: mv[n]
        except (TypeError, ValueError):
            pass
    return lambda n: g.get_property(n, key) or 0

"""FinBench complex-read queries — temporal traversals over the financial graph.

Each rel's timestamp/amount is read mid-traversal via
``g.relationships(node, dir, [rel])`` -> ``rel.get_property("ts" / "amt")``; the
neighbor is the OTHER endpoint (``end_node`` for Outgoing, ``start_node`` for
Incoming). Mirrors src/finbench.rs.
"""

from collections import deque

from rustychickpeas import Direction

_I64_MIN = -(2 ** 63)
MAX_CYCLE_LEN = 6
MAX_CYCLES = 1000


def _ts(rel):
    v = rel.get_property("ts")
    return _I64_MIN if v is None else v


def _amt(rel):
    v = rel.get_property("amt")
    return 0.0 if v is None else v


def trace_transfers_in(g, account, start_ms, end_ms, max_hops):
    """Upstream accounts feeding into ``account`` by a <=max_hops, in-window reverse
    ``transfer`` BFS (TCR1's reverse trace)."""
    visited = {account}
    queue = deque([(account, 0)])
    reached = []
    while queue:
        node, depth = queue.popleft()
        if depth >= max_hops:
            continue
        for rel in g.relationships(node, Direction.Incoming, ["transfer"]):
            ts = _ts(rel)
            if ts < start_ms or ts > end_ms:
                continue
            nbr = rel.start_node().id()  # incoming: neighbor is the source
            if nbr not in visited:
                visited.add(nbr)
                reached.append(nbr)
                queue.append((nbr, depth + 1))
    return reached


def transfer_cycles(g, account, min_amount, window_ms):
    """TCR8 — fund-transfer cycles back to ``account`` where each hop is strictly
    later in time, each amount >= ``min_amount``, completing within ``window_ms``."""
    cycles = []
    path = [account]
    on_path = {account}
    _cycle_dfs(g, account, account, _I64_MIN, None, min_amount, window_ms, path, on_path, cycles)
    return cycles


def _cycle_dfs(g, start, node, last_ts, first_ts, min_amount, window_ms, path, on_path, out):
    if len(path) > MAX_CYCLE_LEN or len(out) >= MAX_CYCLES:
        return
    for rel in g.relationships(node, Direction.Outgoing, ["transfer"]):
        ts = _ts(rel)
        if ts <= last_ts or _amt(rel) < min_amount:
            continue  # strictly increasing time + amount threshold
        f0 = ts if first_ts is None else first_ts
        if ts - f0 > window_ms:
            continue
        nbr = rel.end_node().id()  # outgoing: neighbor is the dest
        if nbr == start:
            if len(path) >= 2:
                out.append(list(path))
            continue
        if nbr in on_path:
            continue
        path.append(nbr)
        on_path.add(nbr)
        _cycle_dfs(g, start, nbr, ts, f0, min_amount, window_ms, path, on_path, out)
        path.pop()
        on_path.remove(nbr)


def shortest_transfer_path(g, src, dst, start_ms, end_ms):
    """TCR3 — shortest in-window ``transfer`` path (hop count) from src to dst, or
    -1 if unreachable. Unweighted BFS over in-window transfer rels."""
    if src == dst:
        return 0
    visited = {src}
    queue = deque([(src, 0)])
    while queue:
        node, d = queue.popleft()
        for rel in g.relationships(node, Direction.Outgoing, ["transfer"]):
            ts = _ts(rel)
            if ts < start_ms or ts > end_ms:
                continue
            nbr = rel.end_node().id()
            if nbr == dst:
                return d + 1
            if nbr not in visited:
                visited.add(nbr)
                queue.append((nbr, d + 1))
    return -1


def guarantee_exposure(g, person):
    """TCR11 — a person's loan exposure: walk the ``guarantee`` chain out from
    ``person``, summing the ``apply`` loan amounts they and their guarantees owe."""
    visited = {person}
    queue = deque([person])
    total = 0.0
    while queue:
        p = queue.popleft()
        for rel in g.relationships(p, Direction.Outgoing, ["apply"]):
            total += _amt(rel)
        for rel in g.relationships(p, Direction.Outgoing, ["guarantee"]):
            nbr = rel.end_node().id()
            if nbr not in visited:
                visited.add(nbr)
                queue.append(nbr)
    return total

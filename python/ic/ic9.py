"""IC9 — the 20 most recent messages by the seed's friends and friends-of-friends
(1..=2 ``knows`` hops, excluding self), on/before max_day.

Returns ``(message_id, ms)`` by (ms desc, id asc), top 20. The seed can be a hub,
so the FoF message set is huge: day/ms are read through the dense column buffers
(O(1), no per-node call) and only the top 20 are kept in a bounded heap rather than
collecting and sorting millions of candidates.
"""

import heapq

from rustychickpeas import Direction

from ._cols import i64_reader


def ic9_fof_messages(g, person, max_day):
    day = i64_reader(g, "day")
    ms = i64_reader(g, "ms")
    heap = []  # min-heap of (ms, -id); keeps the 20 largest by (ms desc, id asc)
    for p in g.neighborhood(person, Direction.Outgoing, "knows", 2, min_hops=1):
        for msg in g.neighbor_ids(p, Direction.Outgoing, ["hasCreator"]):
            if day(msg) <= max_day:
                key = (ms(msg), -msg)
                if len(heap) < 20:
                    heapq.heappush(heap, key)
                elif key > heap[0]:
                    heapq.heapreplace(heap, key)
    return [(-nid, t) for (t, nid) in sorted(heap, reverse=True)]

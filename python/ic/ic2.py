"""IC2 — the 20 most recent messages created by the seed's direct friends,
on/before max_day.

Returns ``(message_id, ms)`` by (ms desc, id asc), top 20. day/ms are read through
the dense column buffers (O(1)) and only the top 20 are kept in a bounded heap.
"""

import heapq

from rustychickpeas import Direction

from ._cols import i64_reader


def ic2_recent_messages(g, person, max_day):
    day = i64_reader(g, "day")
    ms = i64_reader(g, "ms")
    heap = []  # min-heap of (ms, -id); keeps the 20 largest by (ms desc, id asc)
    for friend in g.neighbor_ids(person, Direction.Outgoing, ["knows"]):
        for msg in g.neighbor_ids(friend, Direction.Outgoing, ["hasCreator"]):
            if day(msg) <= max_day:
                key = (ms(msg), -msg)
                if len(heap) < 20:
                    heapq.heappush(heap, key)
                elif key > heap[0]:
                    heapq.heapreplace(heap, key)
    return [(-nid, t) for (t, nid) in sorted(heap, reverse=True)]

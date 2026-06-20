"""IC8 — the 20 most recent replies to the start person's messages, ordered by
(replyCreationDate desc, reply id). Returns (reply_id, ms).
"""

import heapq

from rustychickpeas import Direction

from ._cols import i64_reader


def ic8_recent_replies(g, person):
    ms = i64_reader(g, "ms")
    heap = []  # min-heap of (ms, -id); keeps the 20 largest by (ms desc, id asc)
    for msg in g.neighbor_ids(person, Direction.Outgoing, ["hasCreator"]):
        for reply in g.neighbor_ids(msg, Direction.Incoming, ["replyOf"]):
            key = (ms(reply), -reply)
            if len(heap) < 20:
                heapq.heappush(heap, key)
            elif key > heap[0]:
                heapq.heapreplace(heap, key)
    return [(-nid, t) for (t, nid) in sorted(heap, reverse=True)]

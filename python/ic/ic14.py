"""IC14 — weighted shortest-path cost between two persons over ``knows``, where
each rel costs ``1 / (reply_interactions + 1)`` (more interaction = cheaper).
Returns the path cost, or ``None`` if unreachable.

The interaction weights are the same one-mode projection as BI Q19 (``fold_via``
of ``replyOf`` through each message's creator); a native single-source ``dijkstra``
with ``base=1`` (every rel traversable) and an early-exit ``target`` reads them
without a per-rel Python callback.
"""

from rustychickpeas import Direction


def build_interaction(g):
    creators = g.neighbor_via("hasCreator", Direction.Incoming)
    return g.fold_via("replyOf", Direction.Outgoing, creators)


def ic14_weighted_path(g, p1, p2, interaction):
    if p1 == p2:
        return 0.0
    dist = g.dijkstra(p1, Direction.Outgoing, "knows",
                      weights=interaction, base=1.0, prune_missing=False, target=p2)
    return dist.get(p2)

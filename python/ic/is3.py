"""IS3 — a person's direct ``knows`` friends.

Returns the friend node ids; the comparable projection re-sorts by external id.
"""

from rustychickpeas import Direction


def is3_friends(g, person):
    return list(g.neighbor_ids(person, Direction.Outgoing, ["knows"]))

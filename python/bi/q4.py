"""BI Q4 — top message creators in a country.

Take the top-100 forums created after ``after_day`` ranked by single-country
membership (each forum counted under the country its members live in), then rank
those forums' members by how many messages they created in the forums' post
reply-trees. Reports ``(person_id, message_count)`` top-100, plus the sorted
forum ids (the cross-check uses person id + count and the forum-id set). Ids are
the LDBC external ids (the ``id`` property).

Step 1 is the native ``neighbor_groups(...).project(...).top_by_size(...)`` builder
(the forums whose members form the largest single-country cohort). Step 2 uses
``neighborhood`` for each post's reply-tree and ``first_neighbor`` to attribute
each message to its creator.
"""

from rustychickpeas import Direction

# replyOf is a finite forest, so a large bound just walks to the leaves.
_REPLY_DEPTH = 100_000
_TO_COUNTRY = [(Direction.Outgoing, "isLocatedIn"), (Direction.Outgoing, "isPartOf")]


def q4_top_creators(g, after_day: int):
    # Step 1: the top-100 forums (created after the cutoff) by their largest
    # single-country membership. A forum's place is set by its biggest cohort, so
    # rank by that max, ties by forum id ("id") — matches ranking over every
    # (country, forum) pair, since the country id only tie-breaks same-forum pairs.
    forums = [f for f in g.nodes_with_label("Forum")
              if (g.get_property(f, "fday") or 0) > after_day]
    top = (g.neighbor_groups(forums, "hasMember", Direction.Outgoing)
             .project(_TO_COUNTRY)
             .top_by_size(100, tie="id"))
    top_forums = [f for f, _ in top]

    # Step 2: members of the top forums, ranked by the messages they created in
    # those forums' post reply-trees.
    members = set()
    for f in top_forums:
        members.update(g.neighbor_ids(f, Direction.Outgoing, ["hasMember"]))

    msg_count = {}  # member -> messages they created in the top forums' trees
    for f in top_forums:
        for post in g.neighbor_ids(f, Direction.Outgoing, ["containerOf"]):
            tree = g.neighborhood(post, Direction.Incoming, "replyOf", _REPLY_DEPTH)
            for n in (post, *tree):
                creator = g.first_neighbor(n, Direction.Incoming, "hasCreator")
                if creator is not None and creator in members:
                    msg_count[creator] = msg_count.get(creator, 0) + 1

    rows = sorted(
        ((g.get_property(p, "id"), msg_count.get(p, 0)) for p in members),
        key=lambda r: (-r[1], r[0]),
    )[:100]
    top_ids = sorted(g.get_property(f, "id") for f in top_forums)
    return rows, top_ids

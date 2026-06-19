"""BI Q4 — top message creators in a country.

Take the top-100 forums created after ``after_day`` ranked by single-country
membership (each forum counted under the country its members live in), then rank
those forums' members by how many messages they created in the forums' post
reply-trees. Reports ``(person_id, message_count)`` top-100, plus the sorted
forum ids (the cross-check uses person id + count and the forum-id set). Ids are
the LDBC external ids (the ``id`` property).

First cut on existing primitives: ``follow`` for person->city->country (memoized
per member), ``neighborhood`` for each post's reply-tree, and ``first_neighbor``
to attribute each message to its creator.
"""

from rustychickpeas import Direction

# replyOf is a finite forest, so a large bound just walks to the leaves.
_REPLY_DEPTH = 100_000
_TO_COUNTRY = [(Direction.Outgoing, "isLocatedIn"), (Direction.Outgoing, "isPartOf")]


def q4_top_creators(g, after_day: int):
    # Step 1: count members per (country, forum) for forums created after the cutoff.
    cf = {}  # (country, forum) -> member count
    country_of = {}  # person -> country node (memoized; a member joins many forums)
    for forum in g.nodes_with_label("Forum"):
        if (g.get_property(forum, "fday") or 0) <= after_day:
            continue
        for m in g.neighbor_ids(forum, Direction.Outgoing, ["hasMember"]):
            if m not in country_of:
                country_of[m] = g.follow(m, _TO_COUNTRY)
            country = country_of[m]
            if country is not None:
                key = (country, forum)
                cf[key] = cf.get(key, 0) + 1

    # Rank forums by their single largest country membership, then forum id. A
    # forum's place is set by its best (country, forum) pair, so collapse to that
    # max and sort the forums directly instead of sorting every pair (the country
    # id only ever tie-breaks pairs of the *same* forum, so it can't reorder forums).
    best = {}  # forum -> max member count in any one country
    for (_country, forum), n in cf.items():
        if n > best.get(forum, 0):
            best[forum] = n
    fid = {f: g.get_property(f, "id") for f in best}
    top_forums = sorted(best, key=lambda f: (-best[f], fid[f]))[:100]

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
    top_ids = sorted(fid[f] for f in top_forums)
    return rows, top_ids

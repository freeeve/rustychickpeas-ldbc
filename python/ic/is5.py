"""IS5 — the creator of a message (the first ``hasCreator`` neighbor)."""

from rustychickpeas import Direction


def is5_message_creator(g, message):
    return g.first_neighbor(message, Direction.Incoming, "hasCreator")

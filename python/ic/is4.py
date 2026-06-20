"""IS4 — a message's (creationMs, content). Needs the loader's ``with_content_text``
option (the ``ctext`` property); image-only Posts fall back to their imageFile.
Returns ``None`` when the message has no stored content text.
"""


def is4_message_content(g, message):
    ctext = g.prop_str(message, "ctext")
    if ctext is None:
        return None
    return (g.get_property(message, "ms"), ctext)

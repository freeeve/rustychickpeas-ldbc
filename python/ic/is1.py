"""IS1 — a person's profile: (firstName, lastName, creation day)."""


def is1_profile(g, person):
    return (g.prop_str(person, "fname"), g.prop_str(person, "lname"),
            g.get_property(person, "pday"))

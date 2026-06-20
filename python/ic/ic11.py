"""IC11 — job referral. The seed's <=2-hop ``knows`` neighborhood who worked
(workFrom < ``year``) at a Company located in ``country_name``. Ordered
(workFrom asc, id asc, company name desc), top 10. Returns (person, company, work_from).
"""

from rustychickpeas import Direction


def ic11_job_referral(g, person, country_name, year):
    country = g.node_with_label_property("Country", "name", country_name)
    if country is None:
        return []
    # The country itself and every city in it.
    places = {country}
    places.update(g.neighbor_ids(country, Direction.Incoming, ["isPartOf"]))
    in_country = {
        org for org in g.nodes_with_label("Company")
        if any(pl in places for pl in g.neighbor_ids(org, Direction.Outgoing, ["orgPlace"]))
    }
    rows = []
    for p in g.neighborhood(person, Direction.Outgoing, "knows", 2, min_hops=1):
        for rel in g.relationships(p, Direction.Outgoing, ["workAt"]):
            company = rel.end_node().id()
            if company not in in_country:
                continue
            wf = rel.get_property("wf")
            if wf is None or wf >= year:
                continue
            rows.append((p, company, wf))
    # (workFrom asc, person id asc, company name desc): name desc first, then a
    # stable sort by (wf, id) ascending.
    rows.sort(key=lambda r: g.prop_str(r[1], "name") or "", reverse=True)
    rows.sort(key=lambda r: (r[2], g.get_property(r[0], "id")))
    return rows[:10]

# 038 — SPB advanced q15

**Goal.** Faithful hand-coded rustychickpeas implementation of SPB advanced q15 (no SPARQL engine), matching the official query template.

**Source SPARQL:** `data/spb/ldbc_spb_bm_2.0/datasets_and_queries/sparql/advanced/aggregation_standard/query15.txt` (local SPB checkout; the `{{{...}}}` placeholders are substitution parameters).

**SPB query spec (verbatim header):**
```
 Query name : query15
 Query Description : 
 Retrieve a list of N creative works that are about and mention things of same entity type, additional properties like title, category.
 Additional constraint is added by a filter that selects creative works containing a word which is not so commonly used e.g. "policy" in their titlle.
 Possible point for optimisation, optimizer to eliminate the optional
 (star query)
```

**Steps.**
1. Read the SPARQL template and its substitution parameters.
2. Translate to a hand-coded scan / traversal / aggregation over the SPB property graph (real vocab: `cwork:about`/`mentions`/`title`/`description`/`category`/`audience`/`dateCreated`/`dateModified`; geonames `Feature` with wgs84 `lat`/`long`).
3. Reproduce the result shape (ORDER BY / LIMIT / COUNT / the CONSTRUCT subgraph as rows).
4. Cross-check against the same SPARQL on the Oxigraph store (`data/spb/oxigraph-store`, :7878) over the SPB-10 extract.

**Acceptance.** Results match the Oxigraph SPARQL run on the SPB-10 extract; timed with `time_query`. Flag explicitly any part needing capabilities we lack (RDFS / `owl:sameAs` / `owl:ObjectProperty` reasoning); full-text and geo are covered by 011/012.

**Depends on.** 014 (SPB real-vocab harness); 011/012 (fts/geo, done).

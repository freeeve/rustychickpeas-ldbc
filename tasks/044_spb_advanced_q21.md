# 044 — SPB advanced q21

**Goal.** Faithful hand-coded rustychickpeas implementation of SPB advanced q21 (no SPARQL engine), matching the official query template.

**Source SPARQL:** `data/spb/ldbc_spb_bm_2.0/datasets_and_queries/sparql/advanced/aggregation_standard/query21.txt` (local SPB checkout; the `{{{...}}}` placeholders are substitution parameters).

**SPB query spec (verbatim header):**
```
 Query name : query21
 Query Description : 
 A faceted search query, several iterations of that query follow, adding up more constraints to previous iterations:
   - FTS search on a random word in Creative Works' titles and adding a category type constraint
   - Group by year and month of creation
   - Group by tag (tagged entity being mentioned or tagged about)
   - Group by primary format
   - Finally selecting title and date of creation by adding a constraint on: specific date (day,month,year)
 All query iterations from current drill-down will have same query IDs!
```

**Steps.**
1. Read the SPARQL template and its substitution parameters.
2. Translate to a hand-coded scan / traversal / aggregation over the SPB property graph (real vocab: `cwork:about`/`mentions`/`title`/`description`/`category`/`audience`/`dateCreated`/`dateModified`; geonames `Feature` with wgs84 `lat`/`long`).
3. Reproduce the result shape (ORDER BY / LIMIT / COUNT / the CONSTRUCT subgraph as rows).
4. Cross-check against the same SPARQL on the Oxigraph store (`data/spb/oxigraph-store`, :7878) over the SPB-10 extract.

**Acceptance.** Results match the Oxigraph SPARQL run on the SPB-10 extract; timed with `time_query`. Flag explicitly any part needing capabilities we lack (RDFS / `owl:sameAs` / `owl:ObjectProperty` reasoning); full-text and geo are covered by 011/012.

**Depends on.** 014 (SPB real-vocab harness); 011/012 (fts/geo, done).

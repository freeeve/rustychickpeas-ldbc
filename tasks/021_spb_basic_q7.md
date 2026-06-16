# 021 — SPB basic q7

**Goal.** Faithful hand-coded rustychickpeas implementation of SPB basic q7 (no SPARQL engine), matching the official query template.

**Source SPARQL:** `data/spb/ldbc_spb_bm_2.0/dist/data/sparql/aggregation/query7.txt` (local SPB checkout; the `{{{...}}}` placeholders are substitution parameters).

**SPB query spec (verbatim header):**
```
 Query name : query7
 Query Description : 
 Retrieve creative works that have been created within a defined date-time range of one hour.
 Additional constraint added the type of creative works created in that time range.
 Choke Points : 
   - Date range query
   - Optimizer should be able to split the FILTER conditions into conjunction of conditions and execute them as soon as possible, 
     which will limit the amount of intermediate results
```

**Steps.**
1. Read the SPARQL template and its substitution parameters.
2. Translate to a hand-coded scan / traversal / aggregation over the SPB property graph (real vocab: `cwork:about`/`mentions`/`title`/`description`/`category`/`audience`/`dateCreated`/`dateModified`; geonames `Feature` with wgs84 `lat`/`long`).
3. Reproduce the result shape (ORDER BY / LIMIT / COUNT / the CONSTRUCT subgraph as rows).
4. Cross-check against the same SPARQL on the Oxigraph store (`data/spb/oxigraph-store`, :7878) over the SPB-10 extract.

**Acceptance.** Results match the Oxigraph SPARQL run on the SPB-10 extract; timed with `time_query`. Flag explicitly any part needing capabilities we lack (RDFS / `owl:sameAs` / `owl:ObjectProperty` reasoning); full-text and geo are covered by 011/012.

**Depends on.** 014 (SPB real-vocab harness); 011/012 (fts/geo, done).

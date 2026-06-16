# 019 — SPB basic q5

**Goal.** Faithful hand-coded rustychickpeas implementation of SPB basic q5 (no SPARQL engine), matching the official query template.

**Source SPARQL:** `data/spb/ldbc_spb_bm_2.0/dist/data/sparql/aggregation/query5.txt` (local SPB checkout; the `{{{...}}}` placeholders are substitution parameters).

**SPB query spec (verbatim header):**
```
 Query name : query5
 Query Description : 
 Retrieve entities that are most tagged within one hour interval
 Restriction on audience type and Creative Work type further limits result.
 Choke Points : 
   - Full scan query
     Optimizer should not consider the ORDER BY as important clause in cases where all results are counted (COUNT(*))
   - A sub-select which aggregates results. Optimizer should recognize it and execute it first
   - Join ordering based on cardinality of functional property cwork:dateModified
     Optimizer should use an efficient cost evaluation method for choosing the optimal join tree
   - Optimizer should be able to split the FILTER conditions into conjunction of conditions and execute the as soon as possible,
     which will limit the amount of intermediate results
```

**Steps.**
1. Read the SPARQL template and its substitution parameters.
2. Translate to a hand-coded scan / traversal / aggregation over the SPB property graph (real vocab: `cwork:about`/`mentions`/`title`/`description`/`category`/`audience`/`dateCreated`/`dateModified`; geonames `Feature` with wgs84 `lat`/`long`).
3. Reproduce the result shape (ORDER BY / LIMIT / COUNT / the CONSTRUCT subgraph as rows).
4. Cross-check against the same SPARQL on the Oxigraph store (`data/spb/oxigraph-store`, :7878) over the SPB-10 extract.

**Acceptance.** Results match the Oxigraph SPARQL run on the SPB-10 extract; timed with `time_query`. Flag explicitly any part needing capabilities we lack (RDFS / `owl:sameAs` / `owl:ObjectProperty` reasoning); full-text and geo are covered by 011/012.

**Depends on.** 014 (SPB real-vocab harness); 011/012 (fts/geo, done).

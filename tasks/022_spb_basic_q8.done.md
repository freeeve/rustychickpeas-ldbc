# 022 — SPB basic q8

**Goal.** Faithful hand-coded rustychickpeas implementation of SPB basic q8 (no SPARQL engine), matching the official query template.

**Source SPARQL:** `data/spb/ldbc_spb_bm_2.0/dist/data/sparql/aggregation/query8.txt` (local SPB checkout; the `{{{...}}}` placeholders are substitution parameters).

**SPB query spec (verbatim header):**
```
 Query name : query8
 Query Description : 
 Retrieve creative works and their properties, which contain a a certain word in their title or description. 
 A Full-text search query.
 Choke Points : 
   - OPTIONAL clauses (treated by query optimizer as nested sub-queries)
     Optimizer should decide to put optional triples on top of the join tree 
     (i.e. delay their execution to the last possible moment) because OPTIONALs are treated as a left join
   - Optimizer should be able to split the FILTER conditions into conjunction of conditions and
     start their execution as soon as possible thus eliminating intermediate results
   - A possibility for optimizing the full-text search by using appropriate index
```

**Steps.**
1. Read the SPARQL template and its substitution parameters.
2. Translate to a hand-coded scan / traversal / aggregation over the SPB property graph (real vocab: `cwork:about`/`mentions`/`title`/`description`/`category`/`audience`/`dateCreated`/`dateModified`; geonames `Feature` with wgs84 `lat`/`long`).
3. Reproduce the result shape (ORDER BY / LIMIT / COUNT / the CONSTRUCT subgraph as rows).
4. Cross-check against the same SPARQL on the Oxigraph store (`data/spb/oxigraph-store`, :7878) over the SPB-10 extract.

**Acceptance.** Results match the Oxigraph SPARQL run on the SPB-10 extract; timed with `time_query`. Flag explicitly any part needing capabilities we lack (RDFS / `owl:sameAs` / `owl:ObjectProperty` reasoning); full-text and geo are covered by 011/012.

**Depends on.** 014 (SPB real-vocab harness); 011/012 (fts/geo, done).

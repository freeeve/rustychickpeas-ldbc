# 017 — SPB basic q3

**Goal.** Faithful hand-coded rustychickpeas implementation of SPB basic q3 (no SPARQL engine), matching the official query template.

**Source SPARQL:** `data/spb/ldbc_spb_bm_2.0/dist/data/sparql/aggregation/query3.txt` (local SPB checkout; the `{{{...}}}` placeholders are substitution parameters).

**SPB query spec (verbatim header):**
```
 Query name : query3
 Query Description : 
 Describes all creative works about a topic with certain fixed properties and order them by creation date. The size of the resultset is limited by a random number between 5 and 20.
 Choke Points : 
   - UNIONS - optimizer should execute the UNIONs in terms or in parallel
   - OPTIONAL clauses (treated by query optimizer as nested sub-queries)
     Optimizer should recognize that FILTER condition contains variables which are part of the OPTINAL clauses
     and start execution of OPTIONAL clause as soon as possible thus eliminating the intermediate results.
   - Optimizer should be able to split the FILTER conditions into conjunction of conditions and
     start their execution as soon as possible thus eliminating intermediate results
   - Optimizer could consider the possibility to choose a query plan that would facilitate the ordering (ORDER BY) of result
```

**Steps.**
1. Read the SPARQL template and its substitution parameters.
2. Translate to a hand-coded scan / traversal / aggregation over the SPB property graph (real vocab: `cwork:about`/`mentions`/`title`/`description`/`category`/`audience`/`dateCreated`/`dateModified`; geonames `Feature` with wgs84 `lat`/`long`).
3. Reproduce the result shape (ORDER BY / LIMIT / COUNT / the CONSTRUCT subgraph as rows).
4. Cross-check against the same SPARQL on the Oxigraph store (`data/spb/oxigraph-store`, :7878) over the SPB-10 extract.

**Acceptance.** Results match the Oxigraph SPARQL run on the SPB-10 extract; timed with `time_query`. Flag explicitly any part needing capabilities we lack (RDFS / `owl:sameAs` / `owl:ObjectProperty` reasoning); full-text and geo are covered by 011/012.

**Depends on.** 014 (SPB real-vocab harness); 011/012 (fts/geo, done).

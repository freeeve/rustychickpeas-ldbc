# 023 — SPB basic q9

**Goal.** Faithful hand-coded rustychickpeas implementation of SPB basic q9 (no SPARQL engine), matching the official query template.

**Source SPARQL:** `data/spb/ldbc_spb_bm_2.0/dist/data/sparql/aggregation/query9.txt` (local SPB checkout; the `{{{...}}}` placeholders are substitution parameters).

**SPB query spec (verbatim header):**
```
 Query name : query9
 Query Description :
 Retrieve most recent Creative Works related to a particular one, namely such that are tagged with the same concepts
 Calculates a score for a particular Creative Work, based on the number of Creative Works that it shares tags with
 The different combinations of cwork:about and cwork:mention count with factors between 0.5 and 2
 When calculating the score, multiplication of results due to owl:sameAs equivalence should be suppressed
 For instance, if only the following two statements are asserted in the repository
     <cw1 cwork:tag e1> and <e1 owl:sameAs e2>
 The query SELECT (COUNT(*) AS ?cnt) { cw1 cwork:tag ?e } should return 1, instead of 2
 Reasoning : rdfs:subPropertyOf reasoning with respect to cwork:tag; owl:sameAs with respect to tags
 Choke Points :
   - Optimizer should consider cardinality of star-shaped sub-queries for choosing the optimal join ordering.
   - Optimizer should identify the possibility of asynchronous execution of the  aggregate sub-queries.
   - Optimizer should consider the selectivity of the DISTINCT for choosing the right execution plan. The distinct's state
     should be shared between threads or should be merged after the top order sort.
   - Engines which support optimized handling owl:sameAs reasoning that allows for control of query results expansion
      can implement this query in a much simpler and efficient way. The first sub-query may look as follows:
        SELECT (COUNT(*) AS ?cnt_2) 
        WHERE { 
          ?other_cw cwork:about ?oa . 
          <CreativeWorkUri> cwork:about ?oa .
        }  
```

**Steps.**
1. Read the SPARQL template and its substitution parameters.
2. Translate to a hand-coded scan / traversal / aggregation over the SPB property graph (real vocab: `cwork:about`/`mentions`/`title`/`description`/`category`/`audience`/`dateCreated`/`dateModified`; geonames `Feature` with wgs84 `lat`/`long`).
3. Reproduce the result shape (ORDER BY / LIMIT / COUNT / the CONSTRUCT subgraph as rows).
4. Cross-check against the same SPARQL on the Oxigraph store (`data/spb/oxigraph-store`, :7878) over the SPB-10 extract.

**Acceptance.** Results match the Oxigraph SPARQL run on the SPB-10 extract; timed with `time_query`. Flag explicitly any part needing capabilities we lack (RDFS / `owl:sameAs` / `owl:ObjectProperty` reasoning); full-text and geo are covered by 011/012.

**Depends on.** 014 (SPB real-vocab harness); 011/012 (fts/geo, done).

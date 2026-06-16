# 016 — SPB basic q2

**Goal.** Faithful hand-coded rustychickpeas implementation of SPB basic q2 (no SPARQL engine), matching the official query template.

**Source SPARQL:** `data/spb/ldbc_spb_bm_2.0/dist/data/sparql/aggregation/query2.txt` (local SPB checkout; the `{{{...}}}` placeholders are substitution parameters).

**SPB query spec (verbatim header):**
```
 Query name : query2
 Query Description : 
 Retrieve proeprties of a concrete creative work.
 reasoning rdfs:subClassOf, rdf:type
 join ordering: cwork:dateModified rdf:type owl:FunctionalProperty
 join ordering: cwork:dateCreated rdf:type owl:FunctionalProperty
 optimization: ?creativeWork a ?type . ?type rdfs:subClassOf cwork:CreativeWork 
 should be eliminated since ?creativeWork a cwork:CreativeWork 
 Choke Points : 
   - join ordering based on cardinality of functional proerties cwork:dateCreated, cwork:dateModified
     Optimizer should use an efficient cost evaluation method for choosing the optimal join tree
   - OPTIONAL clauses (treated by query optimizer as nested sub-queries)
     Optimizer should recognize that FILTER condition contains variables which are part of the OPTINAL clauses
     and unlike query1 to start execution of OPTIONAL clause as soon as possible thus eliminating the intermediate results.
   - query optimizer has the chance to recognize the triple pattern : ?creativeWork a ?type . ?type rdfs:subClassOf cwork:CreativeWork 
     and eliminate first triple (?creativeWork a ?type .) since ?creativeWork is a cwork:CreativeWork
```

**Steps.**
1. Read the SPARQL template and its substitution parameters.
2. Translate to a hand-coded scan / traversal / aggregation over the SPB property graph (real vocab: `cwork:about`/`mentions`/`title`/`description`/`category`/`audience`/`dateCreated`/`dateModified`; geonames `Feature` with wgs84 `lat`/`long`).
3. Reproduce the result shape (ORDER BY / LIMIT / COUNT / the CONSTRUCT subgraph as rows).
4. Cross-check against the same SPARQL on the Oxigraph store (`data/spb/oxigraph-store`, :7878) over the SPB-10 extract.

**Acceptance.** Results match the Oxigraph SPARQL run on the SPB-10 extract; timed with `time_query`. Flag explicitly any part needing capabilities we lack (RDFS / `owl:sameAs` / `owl:ObjectProperty` reasoning); full-text and geo are covered by 011/012.

**Depends on.** 014 (SPB real-vocab harness); 011/012 (fts/geo, done).

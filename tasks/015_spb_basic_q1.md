# 015 — SPB basic q1

**Goal.** Faithful hand-coded rustychickpeas implementation of SPB basic q1 (no SPARQL engine), matching the official query template.

**Source SPARQL:** `data/spb/ldbc_spb_bm_2.0/dist/data/sparql/aggregation/query1.txt` (local SPB checkout; the `{{{...}}}` placeholders are substitution parameters).

**SPB query spec (verbatim header):**
```
 Query Name : query1
 Query Description : 
 Retrieve creative works about thing t (or that mention t)
 reasoning: rdfs:subClassOf, rdf:type
 join ordering: cwork:dateModified rdf:type owl:FunctionalProperty
 join ordering: cwork:dateCreated rdf:type  owl:FunctionalProperty  
 Choke Points : 
   - join ordering based on cardinality of functional properties cwork:dateCreated, cwork:dateModified
     Optimizer should use an efficient cost evaluation method for choosing the optimal join tree
   - A sub-select which aggregates results. Optimizer should recognize it and execute it first
   - OPTIONAL and nested OPTIONAL clauses (treated by query optimizer as nested sub-queries)
     Optimizer should decide to put optional triples on top of the join tree 
     (i.e. delay their execution to the last possible moment) because OPTIONALs are treated as a left join
   - query optimizer has the chance to recognize the triple pattern : ?cWork a ?type . ?type rdfs:subClassOf cwork:CreativeWork 
     and eliminate first triple (?cwork a ?type .) since ?cwork is a cwork:CreativeWork
```

**Steps.**
1. Read the SPARQL template and its substitution parameters.
2. Translate to a hand-coded scan / traversal / aggregation over the SPB property graph (real vocab: `cwork:about`/`mentions`/`title`/`description`/`category`/`audience`/`dateCreated`/`dateModified`; geonames `Feature` with wgs84 `lat`/`long`).
3. Reproduce the result shape (ORDER BY / LIMIT / COUNT / the CONSTRUCT subgraph as rows).
4. Cross-check against the same SPARQL on the Oxigraph store (`data/spb/oxigraph-store`, :7878) over the SPB-10 extract.

**Acceptance.** Results match the Oxigraph SPARQL run on the SPB-10 extract; timed with `time_query`. Flag explicitly any part needing capabilities we lack (RDFS / `owl:sameAs` / `owl:ObjectProperty` reasoning); full-text and geo are covered by 011/012.

**Depends on.** 014 (SPB real-vocab harness); 011/012 (fts/geo, done).

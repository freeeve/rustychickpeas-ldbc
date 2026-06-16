# 041 — SPB advanced q18

**Goal.** Faithful hand-coded rustychickpeas implementation of SPB advanced q18 (no SPARQL engine), matching the official query template.

**Source SPARQL:** `data/spb/ldbc_spb_bm_2.0/datasets_and_queries/sparql/advanced/aggregation_standard/query18.txt` (local SPB checkout; the `{{{...}}}` placeholders are substitution parameters).

**SPB query spec (verbatim header):**
```
 Query name : query18
 Query Description : 
 Retrieve creative works that have been modified within a defined date-time range. 
 Additional constraint is the type of creative works modified in that time range.
 Picks from that list a creative work, and updates the time range based on its modification time. 
 The time range constraint could narrow down or expand, depending on the previous result.
 A drill-down query which retrieves a list of N creative works that have been modified in a specific time range.
 All query iterations from current drill-down will have same query IDs!
```

**Steps.**
1. Read the SPARQL template and its substitution parameters.
2. Translate to a hand-coded scan / traversal / aggregation over the SPB property graph (real vocab: `cwork:about`/`mentions`/`title`/`description`/`category`/`audience`/`dateCreated`/`dateModified`; geonames `Feature` with wgs84 `lat`/`long`).
3. Reproduce the result shape (ORDER BY / LIMIT / COUNT / the CONSTRUCT subgraph as rows).
4. Cross-check against the same SPARQL on the Oxigraph store (`data/spb/oxigraph-store`, :7878) over the SPB-10 extract.

**Acceptance.** Results match the Oxigraph SPARQL run on the SPB-10 extract; timed with `time_query`. Flag explicitly any part needing capabilities we lack (RDFS / `owl:sameAs` / `owl:ObjectProperty` reasoning); full-text and geo are covered by 011/012.

**Depends on.** 014 (SPB real-vocab harness); 011/012 (fts/geo, done).

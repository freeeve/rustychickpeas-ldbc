# 010 — SPB without a SPARQL engine (RDF -> property graph)

**Goal.** Run a reduced LDBC Semantic Publishing Benchmark by reusing the BI
methodology: parse RDF, map it to our property graph, and hand-translate the
SPARQL aggregation templates into Rust traversals — **no SPARQL engine, no triple
store, no RDF reasoner**.

**Why.** We already do exactly this for BI (hand-translate official Cypher). SPB
is the same move against a different query language. The catch: SPB is natively
*aggregation-heavy over reference datasets*, which is the full-scan shape Kùzu's
columnar engine wins — so SPB tests the engine's weak axis, except for its
ontology-hierarchy and entity-link traversals. Treat as Tier 3 / optional.

**Depends on.** 001. Stubs in `src/spb.rs`.

**Scope of this task — the BGP / aggregation / hierarchy query subset:**
- RDF parser: **N-Triples** first (`<s> <p> <o> .` is line-parseable); Turtle
  later. A serialization parser, not a store.
- RDF -> PG mapping: IRI subject/object -> node; IRI-valued predicate -> typed
  rel; literal-valued predicate -> node property; `rdf:type` -> label.
- Hand-translate the SPARQL **aggregation** query subset (BGP joins + GROUP/
  ORDER/LIMIT + ontology rollup) — `works_about_entity`, `works_by_category_rollup`.
- Named graphs flattened; RDFS entailment materialized or walked explicitly
  (same as BI Q2's TagClass climb).

**Full-text and geo are NOT dropped — they get real core support.** Rather than
approximate, we implement an inverted index (`tasks/011`) and a geo-spatial index
(`tasks/012`) in `rustychickpeas-core`, then the SPB FTS/geo queries land
faithfully in `tasks/013`. See `docs/core-features.md`.

**Steps.**
1. Acquire SPB reference data (GeoNames / DBpedia subsets + BBC ontologies) and
   generate creative works with the SPB data generator; convert to N-Triples.
2. Write the N-Triples -> `GraphBuilder` mapper.
3. Fill the `spb` stubs for the BGP/hierarchy-shaped aggregation queries. The
   full-text and geo query classes are handled in `tasks/011`–`013`, not dropped.
4. Time with `time_query`; expect the aggregation subset to favour scans
   (document it); the FTS/geo subset plays to traversal/index strength.

**Acceptance.**
- N-Triples reference data loads into the property graph with sane counts.
- The aggregation/hierarchy queries run with stable results. Anything still
  needing entailment we did not materialize is listed explicitly (no silent
  omission); full-text/geo are deferred to `tasks/013`, not skipped.

**Decision note.** SPB is feasible without SPARQL/RDF infrastructure. Its
aggregation subset exercises the columnar-favouring scan shape (not our strength),
but the full-text and geo subset drives two genuinely new core capabilities
(`tasks/011`–`012`) — so SPB is worth doing both for coverage breadth and to
extend the engine.

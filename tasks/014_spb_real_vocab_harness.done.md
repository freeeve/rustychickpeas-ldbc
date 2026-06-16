# 014 — SPB real-vocabulary query harness (prerequisite for 015–051)

**Goal.** Move the SPB family from the toy sample vocabulary onto the **real
generated SPB data**, so the per-query tasks can be implemented and cross-checked
faithfully.

**Why.** Real SPB-10 is generated and the load + fts/geo are validated at scale
(`src/bin/spb_real.rs`; see the `spb-real-data-pipeline` memory). But
`src/spb/queries.rs` still targets the sample vocab (`content`/`Place`/`about`).
The real SPB vocab is: works typed `cwork:BlogPost`/`NewsItem`/… (subclasses — we
materialize a `cwork:CreativeWork` supertype in the extract), `cwork:title` /
`shortTitle` / `description` (full-text), `cwork:about` → dbpedia entity,
`cwork:mentions` → geonames `Feature` (wgs84 `lat`/`long`), `cwork:category`,
`cwork:audience`, `cwork:dateCreated` / `dateModified`.

**Scope.**
1. Refactor `src/spb/{queries,mod,loader}.rs` to the real SPB vocabulary (fold in
   or replace `src/bin/spb_real.rs`'s approach).
2. **Extract pipeline** (script it): Oxigraph `CONSTRUCT` of geonames `Feature`
   coords **cast to xsd:double** + the works n-quads → one self-contained `.nt`
   (the steps in the memory). Support both the validation subset and full SPB-10.
3. **Cross-check helper:** run a query's SPARQL on the Oxigraph store
   (`data/spb/oxigraph-store`, served on :7878) and diff against our hand-coded
   result — the SPB analogue of `kuzu/compare.py`.
4. Adapt `kuzu/run_spb.py` projection + queries to the real vocab for the
   head-to-head.

**Acceptance.** Real SPB-10 loads; the geo (q6) and full-text (q8) queries run
end-to-end and cross-check against Oxigraph. Unblocks tasks 015–051.

**Depends on.** 010 (SPB loader), 011/012 (fts/geo) — all done. Real-data pipeline
in memory `spb-real-data-pipeline`.

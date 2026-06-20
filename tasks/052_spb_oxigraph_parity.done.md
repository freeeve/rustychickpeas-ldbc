# 052 — SPB full-suite Oxigraph parity

**Goal.** Cross-check every feasible SPB query against a real reference engine on
the real SPB-10 extract, not just q6/q8. Oxigraph (native SPARQL) is the
reference: the extract is loaded into its own store
(`data/spb/oxigraph-extract`, gitignored) and each query gets an adapted SPARQL
that mirrors the hand-translation's semantics over the materialized vocabulary
(no RDFS/OWL reasoning).

**Pieces.**
1. `src/spb/parity.rs` + `src/bin/spb_parity.rs` — run every query with one
   fixed, data-derived parameter set, LIMITs disabled (`usize::MAX`) so the
   comparison is over full result sets, and emit `results/spb.parity.rust.json`
   (`{params, queries:{name:{kind, rows}}}`). Also prints per-query rust timings.
2. `scripts/spb_parity_sparql/<q>.rq` — one adapted SPARQL per query, `{{param}}`
   placeholders filled from the emitted params.
3. `scripts/spb_parity.py` — fill each template, run `oxigraph query -l <store>`,
   diff (set / tuple-set, order-insensitive) vs the rust rows, print MATCH/DIFF.

**Parameters (data-derived, non-empty on the extract).**
- word=`football`; topic/entityA=`Action_of_25_February_1781` (9457 works);
  entityB=`Ottoman–Portuguese_conflicts_(1538–57)` (3759 shared);
  category=`bbc/category/Company`; audience=`NationalAudience`; cwType=`BlogPost`;
  dateModified window 2011-03-01..2011-06-01; geo London ±0.5°.

**Known real-data divergences to confirm/resolve.**
- `cwork:tag` is ABSENT from the extract → q23 (requires tag) is vacuous.
- q23 also reads `category` as a literal, but real `category` is a URI rel
  (`bbc/category/Company`) — would be empty even with tag present. Flag/fix.

**Acceptance.** Each feasible query MATCHES Oxigraph on the extract (set-equal),
or the divergence is explained (stemming, reasoning, absent predicate) and, where
it is a real-data bug in our query, fixed. q6/q8 already validated vs Kùzu.

**Depends on.** 014 harness; 015–048 queries; real SPB-10 extract + Oxigraph.

**Result.** All 15 feasible queries (q1–q5, q7, a17–a25) MATCH Oxigraph on the
SPB-10 extract, 0 diff. Getting there fixed three causes:
- N-Triples `\uXXXX`/`\UXXXXXXXX` UCHAR escapes were not decoded by the parser
  (the generator escapes non-ASCII IRIs this way) — non-ASCII entities split into
  two nodes. Fixed in `ntriples` + percent-decode in `loader`. → q5/a19/a25.
- `xsd:dateTime` fractional-second trailing zeros (`.150` vs `.15`) — normalized
  in the harness for both sides. → a19.
- The SPB v2.0 generator emits no `cwork:tag`; per the chosen modeling the
  required tag pattern is folded to the `about`/`mentions` topic links the data
  carries (q5 already did this), making q21/q22/q23 meaningful (227/108/2597).
  → a21/a22/a23.

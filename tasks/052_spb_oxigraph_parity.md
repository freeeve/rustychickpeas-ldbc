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
- q23 also reads `category` as a literal, but real `category` is a URI edge
  (`bbc/category/Company`) — would be empty even with tag present. Flag/fix.

**Acceptance.** Each feasible query MATCHES Oxigraph on the extract (set-equal),
or the divergence is explained (stemming, reasoning, absent predicate) and, where
it is a real-data bug in our query, fixed. q6/q8 already validated vs Kùzu.

**Depends on.** 014 harness; 015–048 queries; real SPB-10 extract + Oxigraph.

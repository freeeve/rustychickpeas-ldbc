# 011 — Core feature: full-text index (rustychickpeas-core)

**Goal.** Add a faithful full-text index to `rustychickpeas-core` so SPB's
keyword queries run for real — no `tantivy`, no approximation.

**Why.** SPB's read mix includes full-text search over creative-work content.
Rather than linear-scan around the gap, this is the next core capability the
benchmark drives upstream (cf. the relationship accessor + `dijkstra` the BI
queries drove). Returns `NodeSet`, so it composes with `nodes_with_label` and
traversal.

**Depends on.** 001 (so the SPB driver can call it). Implemented in the sibling
crate, not here.

**Files (in `../rustychickpeas/rustychickpeas-core/`).**
- new `src/fulltext.rs` — inverted index + tokenizer + BM25
- `src/bitmap.rs` — add `NodeSet::{intersect,union,difference}` (RoaringBitmap
  `&`/`|`/`-`; bitvec bitwise) — prerequisite for FTS∩geo∩label composition
- `src/graph_builder.rs` — `index_fulltext(&[(label, property)])`; build at
  `finalize`
- `src/graph_snapshot.rs` — `fts` / `fts_ranked` query methods

**Steps.**
1. `NodeSet` set algebra first (small, broadly useful, unblocks composition).
2. Tokenizer: lowercase, split on non-alphanumeric, optional stopwords; stemming
   deferred. Reuse the `Atoms` interner for the term dictionary where possible.
3. Inverted index `term -> RoaringBitmap<NodeId>`; store tf + doc length only if
   ranking is enabled (gate behind a build flag to keep the boolean path lean).
4. `fts(field, query)` → boolean AND (bitmap intersect) → `NodeSet`.
5. `fts_ranked(field, query, k)` → BM25 over df/tf/doc-length → top-k.
6. Unit + fuzz tests (per repo guidelines): tokenizer round-trips, AND/OR set
   identities, BM25 monotonicity; fuzz the tokenizer on arbitrary UTF-8.

**Acceptance.**
- `fts` returns the correct `NodeSet` on a small fixture; `fts_ranked` orders by
  relevance.
- No new third-party dependency added to core.
- `cargo test` green in core; coverage on the new module >80%.

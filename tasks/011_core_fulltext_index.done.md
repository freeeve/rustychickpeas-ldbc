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
- `src/graph_builder.rs` — `index_fulltext(&[(label, property)])`; build at
  `finalize` by extending the `node_col_str` column scan
- `src/graph_snapshot.rs` — `fts` / `fts_ranked` query methods
- (no `NodeSet` change needed — set algebra `&`/`|`/`-` already exists in
  `src/bitmap.rs`; add named `.intersect()` wrappers only if wanted for clarity)

**Steps.**
1. Tokenizer/analyzer: lowercase, split on non-alphanumeric, optional stopwords;
   stemming deferred. Term dictionary = a dedicated `StringInterner` over tokens
   (distinct from the value interner / `Atoms`).
2. Build at `finalize`: walk `node_col_str[key]` `(node, str_id)` pairs, resolve
   the string, tokenize, and add the node to each token's postings
   (`term -> NodeSet`) — the same column scan the equality index already does.
3. `fts(field, query)` → boolean AND across query terms via `&` on the postings
   `NodeSet`s → `NodeSet`.
4. `fts_ranked(field, query, k)` → BM25 over df (=`postings.len()`) / tf /
   doc-length → top-k. Store the tf + doc-length sidecar only when ranking is
   enabled (build flag), so the boolean path stays lean.
5. Unit + fuzz tests (per repo guidelines): tokenizer round-trips, AND/OR set
   identities, BM25 monotonicity; fuzz the tokenizer on arbitrary UTF-8.

**Acceptance.**
- `fts` returns the correct `NodeSet` on a small fixture; `fts_ranked` orders by
  relevance.
- No new third-party dependency added to core.
- `cargo test` green in core; coverage on the new module >80%.

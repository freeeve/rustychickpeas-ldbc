# 050 — SPB editorial: update

**Goal.** Faithful implementation of the SPB editorial `update` operation against the rustychickpeas SPB graph.

**Source SPARQL Update:** `data/spb/ldbc_spb_bm_2.0/datasets_and_queries/sparql/basic/editorial/update.txt` (local SPB checkout).

**SPB spec (verbatim header):**
```
 Query Description : 
 Updates a Creative Work by first dropping its entire graph and creating a new one
```

**Caveat — mutation.** `GraphSnapshot` is immutable, so editorial `update` does not fit the read-only model directly: stage changes via a `GraphBuilder` delta and re-`finalize`, or add incremental mutation to the engine. The hard part flagged as "maybe / if possible" — scope a spike first.

**Steps.**
1. Read the SPARQL Update template (INSERT/DELETE DATA/WHERE) + substitution params.
2. Decide the mutation model (builder delta + re-finalize vs incremental).
3. Apply the `update`; verify with a follow-up read query.

**Acceptance.** The `update` applies and a verifying read reflects it; cross-checked against the same Update on Oxigraph. May be deferred if mutation support is out of scope.

**Depends on.** 014; mutation/delta support in rustychickpeas-core (likely a new core capability).

## Status: deferred
Blocked on a core mutation/delta API — `GraphSnapshot` is immutable and rustychickpeas-core
has no incremental insert/update/delete path. The SPB editorial writes can't be implemented
faithfully until that capability exists. Reopen when a mutation/delta API lands.

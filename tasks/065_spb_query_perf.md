# 065 — SPB query performance: baseline + optimize cycle

Systematic per-query optimization of the 30 SPB query implementations (our
engine, not the Kùzu reference). Tool: `spb_parity` now installs a counting
allocator (`src/alloc_count.rs`) and its timing table reports median ms,
allocations, bytes and rows per query over the FULL result set.

**Per-query cycle** (parity must stay 30/30 throughout — re-run
`scripts/spb_parity.py` after each change):
1. benchmark allocations (baseline)
2. optimize allocations
3. benchmark again
4. profile CPU (`samply record -- target/release/spb_parity <extract>`)
5. optimize CPU
6. benchmark again

**Baseline** (median of 5, full result set, SPB-10 extract):

| query | ms | allocs | bytes | rows | lead |
|---|--:|--:|--:|--:|---|
| a13 | 82.3 | 710420 | 88 MB | 336315 | `work_uri.clone()`+`tag.to_string()` per pair; sort `(String,String)` |
| a25 | 37.4 | 47704 | 12.6 MB | 47499 | HashSet-per-who; sort-time `pstr` |
| a5 | 37.1 | 108495 | 16.7 MB | 108476 | `has_label` string lookup in loop; per-result `to_string` |
| q3 | 12.2 | 18 | 240 KB | 9457 | CPU: minute extraction/grouping |
| a7 | 9.0 | 33579 | 5.5 MB | 33561 | per-result `to_string`; primaryContentOf count per work |
| a14 | 8.5 | 17 | 2.2 MB | 23192 | CPU: webDocumentType edge traversal per work |
| q5 | 7.9 | 12219 | 1.5 MB | 7898 | `pstr` audience match per neighbour; label join |
| q9 | 7.8 | 51839 | 4.6 MB | 9462 | HashSet intersections per candidate |
| a19 | 7.7 | 27190 | 4.4 MB | 11434 | per-topic accumulation |
| a6 | 6.0 | 6 | 381 B | 3 | CPU: `has_label` ×3 types in inner loop |
| a8 | 2.8 | 11450 | 1.8 MB | 11434 | per-result `to_string` |

Queries below ~2 ms with low allocs (q1/q4/a1/a10/a17/a18/a20–a24/a15/q2/a2) are
already fast; no action unless a shared helper change touches them.

**Shared levers seen across queries:**
- resolve URIs/labels to node ids / `&NodeSet` ONCE, compare ids / bitmap
  `contains` in the loop (kills per-iteration `pstr` / `has_label` string lookups);
- collect node ids `(u32, …)` and resolve `uri` strings only for the final
  (sorted/truncated) output, not during the scan;
- `sort_unstable` / partial-sort when a `limit` is set.

Per-query tasks: 066 a13 · 067 a25 · 068 a5 · 069 q3 · 070 a7 · 071 a14 ·
072 q5 · 073 q9 · 074 a19 · 075 a6 · 076 a8.

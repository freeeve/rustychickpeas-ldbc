# 001 — Extract a shared `src/lib.rs` (prerequisite for all families)

**Goal.** Make the loader and helpers reusable so each benchmark family is a thin
binary, and bring `main.rs` back under the file-size guideline.

**Why.** `src/main.rs` is ~2548 lines (over our `<1000` worst case) and is a
*binary* — its `load_graph`, `Stats`, the property/date helpers and the
`time_query` harness cannot be imported by a sibling family. Every other task in
this set depends on this one.

**Depends on.** Nothing. Do first.

**Files.**
- new `src/lib.rs` — public surface for shared code
- `src/main.rs` -> `src/bin/bi.rs` (thin; calls the lib)
- keep BI query fns either in the bin or a `bi` module under lib

**Steps.**
1. Move into `lib.rs` and `pub`-export: `load_graph`, `Stats`,
   `days_from_civil`, `parse_date`, `parse_ms`, `pi64`, `pbool`, `pstr`, `jstr`,
   `emit_json`, `tag_by_name`, `for_each_row`, `set_message_props`, `time_query`,
   and the `Result` alias.
2. Group them: `loader` (CSV ingest + `Stats`), `props` (typed property/date
   helpers), `harness` (`time_query`, `emit_json`). Keep each file `<500` lines.
3. Convert the current `main()` into `src/bin/bi.rs` that imports the lib; the
   BI query fns (`q1..q20`, `bi1..bi6`) can stay alongside it or move to a
   `bi` module — either way they call shared helpers from the lib.
4. Add `[lib]` to `Cargo.toml`; the existing `[[bin]] ldbc-bench` becomes
   `src/bin/bi.rs` (rename the bin to `ldbc-bi` or keep `ldbc-bench`).

**Acceptance.**
- `cargo build` and `cargo run --bin bi` reproduce the current SF1 output.
- No file over 1000 lines; helpers are `pub` and importable.
- Stub modules `src/{interactive,graphalytics,finbench,spb}.rs` can now be turned
  into `src/bin/*.rs` that `use rustychickpeas_ldbc::...`.

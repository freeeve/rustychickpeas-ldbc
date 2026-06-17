# 100 — IC/IS per-query optimization methodology

Systematic deep-optimization sweep over the Interactive set (IC1–14, IS1–7),
numeric order. Each query gets its own task file (`101_opt_ic1` … `120_opt_is7`)
carrying the loop below; this file holds the shared tooling and commands.

## Why deterministic metrics lead

This box runs under heavy, variable load (7→30 swings mid-run), so **wall-clock
is a noisy optimization signal**. So we optimize against deterministic,
load-independent signals first and only use wall-clock as a same-window
confirmation:

1. **Allocations** (deterministic) — alloc count + bytes per query.
2. **CPU profile** (sampling) — hot-function *shares* are stable even when
   absolute time moves with load.
3. **Wall-clock** — best-of-N, before/after in the *same load window* (stash the
   change between, as in the rank/select A/B), as a final sanity check only.

## The per-query loop

1. **Bench allocations (baseline)** — record allocs/bytes.
2. **CPU profile** — find the hot functions / where the allocs come from.
3. **Optimize** — driven by 1 & 2. Core (`rustychickpeas-core`) changes need
   maintainer sign-off; prefer query-side changes.
4. **Re-bench allocations + re-profile** — confirm the deterministic metrics moved.
5. **Wall-clock A/B** — same-window median, before vs after.
6. **Verify value-identity** — emit + compare vs Kùzu; record before/after in the
   task and mark it `.done`.

## Commands

Harness flags (added in `harness::BenchCfg`): `--only <id>` (e.g. `ic5`),
`--repeat <n>`, `--alloc`. The first non-flag arg stays the snapshot path.

```bash
# 1+4. Allocations (deterministic) — needs the alloc-count feature
cargo run --release --bin ic --features alloc-count -- --only ic5 --alloc
#   -> "IC5 new groups   allocs=N  bytes=M  (result=...)"

# 2. CPU profile (macOS-friendly, no sudo).  cargo install samply  if missing.
cargo build --release --bin ic
samply record -- ./target/release/ic --only ic5 --repeat 300
#   load happens once; with enough repeats the query dominates the samples.
#   fallbacks: `cargo flamegraph --bin ic -- --only ic5 --repeat 300` (dtrace+sudo),
#              `cargo instruments -t time --bin ic -- --only ic5 --repeat 300` (Xcode)

# 5. Wall-clock A/B — run BEFORE and AFTER back-to-back in one load window
#    (git stash the change between, like the rank/select A/B) to cancel load.
cargo run --release --bin ic -- --only ic5 --repeat 25

# 6. Value-identity vs Kùzu (must stay byte-identical)
LDBC_EMIT_JSON=/tmp/xchk cargo run --release --bin ic
.venv-kuzu/bin/python kuzu/run_ic.py sf1 --emit-json /tmp/xchk
.venv-kuzu/bin/python kuzu/compare.py /tmp/xchk ic1 ic2 ic3 ic4 ic5 ic6 ic7 ic8 \
    ic9 ic10 ic11 ic12 ic13 ic14 is1 is2 is3 is5 is6 is7
```

## Ground rules

- Value-identity is non-negotiable: every optimization re-verified byte-identical.
- Don't touch SPB files (concurrent session); stage only this sweep's files.
- A deterministic win with no wall-clock movement is still a win (lower variance,
  less GC/alloc pressure); record it.
- If a query is already minimal (e.g. IS1/IS8 short reads in microseconds), say so
  and close the task — don't manufacture churn.

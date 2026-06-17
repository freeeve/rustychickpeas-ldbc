# 120 — Optimize IS7: replies of message

Per-query optimization loop. Methodology + commands:
[100_ic_perf_methodology.md](100_ic_perf_methodology.md).

## Loop
- [ ] 1. Bench allocations (baseline): `cargo run --release --bin ic --features alloc-count -- --only is7 --alloc`
- [ ] 2. CPU profile: `samply record -- ./target/release/ic --only is7 --repeat 300`
- [ ] 3. Optimize (driven by steps 1+2; core changes need sign-off)
- [ ] 4. Re-bench allocations + re-profile
- [ ] 5. Wall-clock A/B (best-of-N, same load window): `--only is7 --repeat 25`
- [ ] 6. Verify value-identity vs Kùzu; record below; rename to .done

## Measurements
| metric                  | baseline | after |
|-------------------------|----------|-------|
| allocs                  |          |       |
| bytes                   |          |       |
| wall-clock median (ms)  |          |       |
| hot fn (CPU %)          |          |       |

## Notes

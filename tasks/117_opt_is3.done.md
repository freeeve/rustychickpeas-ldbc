# 117 — Optimize IS3: person friends

Closed as **already minimal** (IS batch triage — all IS allocs/timings measured
together; the only notable one was IS2, see tasks/116).

9 allocs / 8 KB, ~0.02 ms — collect the 848 direct friends + sort by id; no
antipattern (the result *is* the full friend list).

**Status: done (no change — already minimal).**

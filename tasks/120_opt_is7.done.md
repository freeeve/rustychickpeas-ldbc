# 120 — Optimize IS7: replies of message

Closed as **already minimal** (IS batch triage — all IS allocs/timings measured
together; the only notable one was IS2, see tasks/116).

10 allocs / 10 KB, ~0.05 ms — the message has 3 replies; the sort is trivial.
(No truncate; a prolific thread could grow, but the reply fan-out is bounded.)

**Status: done (no change — already minimal).**

# 002 — Interactive (IC) query seeds — DONE

`pick_seeds(&GraphSnapshot) -> Option<IcSeeds>` in `src/interactive.rs`
derives reproducible IC parameters from the loaded snapshot (no download):
- start `person`: max `knows` degree, smallest id on ties
- `max_day`: fixed late window (2013-01-01) that contains messages
- `first_name`: most common firstName among the start's <=3-hop friends
  (so IC1 is non-empty), alphabetical tie-break
- `person_b`: farthest person reachable over `knows`, smallest id on ties

Deterministic (degree/id/name sorts) and printed by the `ic` binary for the
record. On SF1: person 17755 (848 friends), firstName "John", person_b 4 hops
away. Required a loader add: Person `fname`/`lname` (additive — BI re-verified
0-diff against the prior baseline).

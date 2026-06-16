# 064 — Optimize IS7 (replies of a message) — DONE (reviewed, no change)

Kùzu IS7 ~150 ms. Reviewed: dominated by scanning the seed's posts to find the
newest (ORDER BY mts DESC LIMIT 1); no Cypher anti-pattern to remove (an index on
(creator, mts) would help but is out of scope). Small absolute, left as-is.

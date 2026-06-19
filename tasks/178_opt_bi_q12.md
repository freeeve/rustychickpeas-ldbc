# 178 — Optimize BI Q12 (message histogram)

Baseline (full SF1, median of 5): Python 946 ms vs Rust 38.8 ms (~24x).
Lead: a full 2.8M-message scan. day/len/content already read via dense-column `memoryview`
(O(1), C-speed), but the Python `for`-loop over 2.8M nodes is itself the floor (~0.5-1s).
The root-post-language check (memoized replyOf walk) and creator attribution only run for
the filtered subset.
Approach: (a) numpy-vectorize the day/len/content mask — but numpy isn't in the venv (would
add a dep); (b) a native columnar filter that returns the surviving message ids, then do
only the root-lang/creator work in Python; (c) push the whole histogram to the `aggregate`
kernel — blocked by the per-message root-post-language graph traversal, which the kernel
can't express. (b) is the most promising native route.

## Result
(pending)

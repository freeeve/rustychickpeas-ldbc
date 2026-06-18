"""Tiny timing harness for the Python BI queries."""

import statistics
import time


def time_query(label: str, runs: int, fn):
    """Run ``fn`` once for its result, then ``runs`` more times for timing.
    Prints the median wall time in ms and returns ``(result, median_ms)``."""
    result = fn()
    times = []
    for _ in range(runs):
        t = time.perf_counter()
        fn()
        times.append((time.perf_counter() - t) * 1000.0)
    med = statistics.median(times)
    print(f"  {label:<34} {med:9.1f} ms  (median of {runs})")
    return result, med

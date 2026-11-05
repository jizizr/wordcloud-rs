#!/usr/bin/env python3
"""Run a command N times; report median wall-clock ms and peak child RSS (MB)."""
import resource, statistics, subprocess, sys, time
n = int(sys.argv[1]); cmd = sys.argv[2:]
times = []
for i in range(n):
    t = time.perf_counter()
    subprocess.run([c.replace("{i}", str(i)) for c in cmd], check=True, stdout=subprocess.DEVNULL)
    times.append((time.perf_counter() - t) * 1000)
rss = resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss / 1024
print(f"median_ms={statistics.median(times):.1f} min_ms={min(times):.1f} peak_rss_mb={rss:.1f}")

#!/usr/bin/env python3
"""Benchmarks the Rust CLI against the Python `wordcloud` package and prints
the README table, together with the machine it ran on.

    cargo build --release
    python3 -m venv /tmp/wc-venv && /tmp/wc-venv/bin/pip install wordcloud==1.9.6
    python3 tools/benchmark.py --python /tmp/wc-venv/bin/python --runs 10 --stress-runs 3

For every workload each tool is run once per seed (0 .. runs-1) as a separate
process; the table reports the median wall-clock time and the median peak
resident memory (RSS) of that process. "Python, in-process" times only
`WordCloud(...).generate(text).to_image()` inside one interpreter, i.e. without
interpreter start-up and imports, which is the fairest comparison for library use.

The stress workloads (huge text, 4K canvas, very dense layout) take minutes in
Python, so they run fewer seeds and only end to end: at these sizes the ~0.5 s
interpreter start-up is negligible.
"""
import argparse
import os
import platform
import statistics
import subprocess
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
GOLDEN = ROOT / "tests" / "golden"
RUST = ROOT / "target" / "release" / "wordcloud"

WORKLOADS = [
    ("Default, 400×200", ["--text", GOLDEN / "alice.txt"], {}),
    (
        "Large, 1000×700, 400 words",
        ["--text", GOLDEN / "alice.txt", "--width", 1000, "--height", 700, "--max_words", 400],
        {"width": 1000, "height": 700, "max_words": 400},
    ),
    (
        "Mask, 900×900, 400 words",
        ["--text", GOLDEN / "alice.txt", "--mask", GOLDEN / "alice_mask.png", "--max_words", 400],
        {"mask": str(GOLDEN / "alice_mask.png"), "max_words": 400},
    ),
]


def stress_workloads(tmp):
    """Huge input text, a 4K canvas and a very dense layout."""
    big = Path(tmp) / "big.txt"
    corpus = "".join((GOLDEN / f).read_text(encoding="utf-8")
                     for f in ["alice.txt", "a_new_hope.txt", "constitution.txt"])
    big.write_text(corpus * (50_000_000 // len(corpus) + 1), encoding="utf-8")
    alice = GOLDEN / "alice.txt"
    return [
        ("50 MB of text, 400×200", ["--text", big]),
        ("4K canvas, 3840×2160, 1000 words", ["--text", alice, "--width", 3840, "--height", 2160,
                                               "--max_words", 1000]),
        ("Very dense, 1000×700, 3000 words, min font 2, margin 1, repeat",
         ["--text", alice, "--width", 1000, "--height", 700, "--max_words", 3000,
          "--min_font_size", 2, "--margin", 1, "--repeat"]),
    ]


IN_PROCESS = r"""
import sys, time, statistics
import numpy as np
from PIL import Image
from wordcloud import WordCloud
text = open(sys.argv[1], encoding="utf-8").read()
kw = eval(sys.argv[2])
if "mask" in kw:
    kw["mask"] = np.array(Image.open(kw["mask"]))
times = []
for seed in range(int(sys.argv[3])):
    t = time.perf_counter()
    WordCloud(random_state=seed, **kw).generate(text).to_image()
    times.append((time.perf_counter() - t) * 1000)
print(statistics.median(times))
"""


def run_once(cmd):
    """Wall-clock ms and peak RSS (MB) of one child process."""
    t = time.perf_counter()
    proc = subprocess.Popen([str(c) for c in cmd], stdout=subprocess.DEVNULL)
    _, status, usage = os.wait4(proc.pid, 0)
    ms = (time.perf_counter() - t) * 1000
    if status != 0:
        raise SystemExit(f"command failed: {cmd}")
    return ms, usage.ru_maxrss / 1024


def measure(base_cmd, args, runs, out):
    times, rss = [], []
    for seed in range(runs):
        ms, mb = run_once([*base_cmd, *args, "--random_state", seed, "--imagefile", out])
        times.append(ms)
        rss.append(mb)
    return statistics.median(times), statistics.median(rss)


def machine(python):
    info = {}
    for line in Path("/proc/cpuinfo").read_text().split("\n\n")[0].splitlines():
        key, _, value = line.partition(":")
        info[key.strip()] = value.strip()
    cpu = info.get("model name", "unknown CPU")
    vm = ", virtual machine" if "hypervisor" in info.get("flags", "").split() else ""
    mem_kb = int(next(l for l in Path("/proc/meminfo").read_text().splitlines()
                      if l.startswith("MemTotal")).split()[1])
    py = subprocess.run([python, "-c", "import sys, wordcloud, numpy, PIL; "
                         "print(sys.version.split()[0], wordcloud.__version__, "
                         "numpy.__version__, PIL.__version__)"],
                        capture_output=True, text=True, check=True).stdout.split()
    rustc = subprocess.run(["rustc", "--version"], capture_output=True, text=True).stdout.split()[1]
    os_name = platform.freedesktop_os_release().get("PRETTY_NAME", platform.system())
    return [
        f"CPU: {cpu} (family {info.get('cpu family')}, model {info.get('model')}, "
        f"stepping {info.get('stepping')}), {os.cpu_count()} cores{vm}",
        f"Memory: {mem_kb / 1024 / 1024:.1f} GiB",
        f"OS: {os_name}, Linux {platform.release()}",
        f"Rust: rustc {rustc}, `cargo build --release` (fat LTO, no PGO)",
        f"Python: {py[0]}, wordcloud {py[1]}, NumPy {py[2]}, Pillow {py[3]}",
    ]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--python", default="python3", help="interpreter with `wordcloud` installed")
    ap.add_argument("--runs", type=int, default=10, help="seeds per workload")
    ap.add_argument("--stress-runs", type=int, default=3, help="seeds per stress workload")
    a = ap.parse_args()

    print("\n".join(f"- {line}" for line in machine(a.python)))
    print(f"- Runs: {a.runs} seeds per workload ({a.stress_runs} for stress tests), "
          "median reported; all runs are single-threaded\n")
    print("| Workload | Python CLI | Python, in-process | Rust CLI | Speed-up (CLI / in-process) "
          "| Peak memory, Python → Rust |")
    print("|---|---:|---:|---:|---:|---:|")
    with tempfile.TemporaryDirectory() as tmp:
        out = Path(tmp) / "out.png"
        for name, args, kw in WORKLOADS:
            py_ms, py_mb = measure([a.python, "-m", "wordcloud"], args, a.runs, out)
            rs_ms, rs_mb = measure([RUST], args, a.runs, out)
            inproc = float(subprocess.run(
                [a.python, "-c", IN_PROCESS, str(GOLDEN / "alice.txt"), repr(kw), str(a.runs)],
                capture_output=True, text=True, check=True).stdout)
            print(f"| {name} | {py_ms:,.0f} ms | {inproc:,.0f} ms | **{rs_ms:,.0f} ms** "
                  f"| **{py_ms / rs_ms:.1f}×** / {inproc / rs_ms:.1f}× "
                  f"| {py_mb:.0f} MB → **{rs_mb:.0f} MB** |")

        print("\n| Stress test | Python CLI | Rust CLI | Speed-up | Peak memory, Python → Rust |")
        print("|---|---:|---:|---:|---:|")
        for name, args in stress_workloads(tmp):
            py_ms, py_mb = measure([a.python, "-m", "wordcloud"], args, a.stress_runs, out)
            rs_ms, rs_mb = measure([RUST], args, a.stress_runs, out)
            print(f"| {name} | {py_ms / 1000:,.1f} s | **{rs_ms / 1000:,.1f} s** "
                  f"| **{py_ms / rs_ms:.1f}×** | {py_mb:,.0f} MB → **{rs_mb:,.0f} MB** |",
                  flush=True)


if __name__ == "__main__":
    main()

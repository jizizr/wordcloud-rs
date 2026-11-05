"""A/B differential parity: legacy Python vs Rust (replaying Pillow glyphs).

Usage: parity.py WORKDIR SEEDS [scenario ...]

Needs a checkout of amueller/word_cloud with its package installed in
`.venv` (set LEGACY_WORDCLOUD, default: ../word_cloud next to this repo) and
a release build of this crate (`cargo build --release`).

For each scenario x seed: run tools/record_pillow.py, then the Rust CLI with
--replay-glyphs, and compare (1) the layout field by field (words, sizes,
positions, orientations, colour strings; frequencies within 1e-12) and
(2) the rendered images pixel by pixel. Prints a JSON summary.
"""
import json
import os
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
LEGACY = os.environ.get("LEGACY_WORDCLOUD", os.path.join(os.path.dirname(ROOT), "word_cloud"))
PY = os.path.join(LEGACY, ".venv", "bin", "python")
RS = os.environ.get("WORDCLOUD_RS", os.path.join(ROOT, "target", "release", "wordcloud"))
EX = os.path.join(LEGACY, "examples")
SCENARIOS = {
    "default": ["--text", f"{EX}/alice.txt"],
    "large": ["--text", f"{EX}/alice.txt", "--width", "1000", "--height", "700", "--max_words", "400"],
    "rs05_repeat": ["--text", f"{EX}/constitution.txt", "--relative_scaling", "0.5", "--repeat", "--max_words", "300"],
    "mask_contour": ["--text", f"{EX}/alice.txt", "--mask", f"{EX}/alice_mask.png", "--contour_width", "3",
                     "--contour_color", "steelblue", "--background", "white", "--max_words", "300"],
    "single_color_vertical": ["--text", f"{EX}/a_new_hope.txt", "--color", "deepskyblue",
                              "--prefer_horizontal", "0.3", "--no_collocations", "--min_word_length", "3"],
    "colormask": ["--text", f"{EX}/alice.txt", "--mask", f"{EX}/alice_color.png", "--colormask",
                  f"{EX}/alice_color.png", "--max_words", "150"],
}


def load_png(path):
    from PIL import Image  # run under the legacy venv
    im = Image.open(path)
    return im.mode, im.size, im.tobytes()


def compare_layout(py, rs):
    if len(py) != len(rs):
        return f"length {len(py)} vs {len(rs)}"
    for i, (a, b) in enumerate(zip(py, rs)):
        for k in ("word", "font_size", "position", "orientation", "color"):
            if a[k] != b[k]:
                return f"word #{i} field {k}: {a[k]!r} vs {b[k]!r}"
        if abs(a["freq"] - b["freq"]) > 1e-12:
            return f"word #{i} freq {a['freq']} vs {b['freq']}"
    return None


def run_case(workdir, name, args, seed):
    prefix = os.path.join(workdir, f"{name}_{seed}")
    full = args + ["--random_state", str(seed)]
    subprocess.run([PY, "-I", os.path.join(ROOT, "tools", "record_pillow.py"), prefix, "--", *full,
                    "--imagefile", prefix + ".py_unused.png"], check=True, cwd=LEGACY)
    r = subprocess.run([RS, *full, "--imagefile", prefix + ".rs.png", "--replay-glyphs", prefix + ".glyphs.json",
                        "--dump-layout", prefix + ".rs.layout.json"], capture_output=True, text=True)
    if r.returncode != 0:
        return {"ok": False, "reason": r.stderr.strip(), "max_abs_px": None}
    py = json.load(open(prefix + ".layout.json"))
    rs = json.load(open(prefix + ".rs.layout.json"))
    reason = compare_layout(py, rs)
    (m1, s1, d1), (m2, s2, d2) = load_png(prefix + ".png"), load_png(prefix + ".rs.png")
    max_px, diff_px = None, None
    if s1 == s2 and len(d1) == len(d2):
        diffs = [abs(a - b) for a, b in zip(d1, d2)]
        max_px, diff_px = max(diffs), sum(1 for d in diffs if d)
        if reason is None and max_px:
            reason = f"{diff_px} channel values differ (max {max_px})"
    elif reason is None:
        reason = f"image size {s1}/{m1} vs {s2}/{m2}"
    return {"ok": reason is None, "reason": reason, "words": len(py), "max_abs_px": max_px, "diff_px": diff_px}


def main():
    workdir, seeds = sys.argv[1], int(sys.argv[2])
    names = sys.argv[3:] or list(SCENARIOS)
    os.makedirs(workdir, exist_ok=True)
    results = {}
    for name in names:
        for seed in range(seeds):
            res = run_case(workdir, name, SCENARIOS[name], seed)
            results[f"{name}/{seed}"] = res
            print(name, seed, "OK" if res["ok"] else "DIVERGE: " + str(res["reason"]), flush=True)
    passed = sum(r["ok"] for r in results.values())
    max_px = max((r["max_abs_px"] or 0) for r in results.values())
    print(json.dumps({"tested": len(results), "passed": passed, "max_abs_px": max_px}))


if __name__ == "__main__":
    main()

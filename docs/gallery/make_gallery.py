#!/usr/bin/env python3
"""Regenerates the README gallery images.

    cargo build --release && python3 docs/gallery/make_gallery.py

Writes docs/gallery/*.png (README.md) and docs/gallery/zh/*.png (README.zh-CN.md).
Needs Pillow for the masks. The Chinese fonts (all SIL OFL) are downloaded into
docs/gallery/_src/fonts/ on first run; set FONT_DIR to use fonts you already have.
"""
import os
import subprocess
import urllib.request
from pathlib import Path

from PIL import Image, ImageDraw

import zh_words

ROOT = Path(__file__).resolve().parents[2]
OUT = ROOT / "docs" / "gallery"
SRC = OUT / "_src"
BIN = ROOT / "target" / "release" / "wordcloud"
GOLDEN = ROOT / "tests" / "golden"
W, H = 600, 400

FONT_DIR = Path(os.environ.get("FONT_DIR", SRC / "fonts"))
GF = "https://raw.githubusercontent.com/google/fonts/main/ofl"
FONTS = {
    "LXGWWenKai-Medium.ttf":
        "https://raw.githubusercontent.com/lxgw/LxgwWenKai/main/fonts/TTF/LXGWWenKai-Medium.ttf",
    "ZCOOLKuaiLe-Regular.ttf": f"{GF}/zcoolkuaile/ZCOOLKuaiLe-Regular.ttf",
    "MaShanZheng-Regular.ttf": f"{GF}/mashanzheng/MaShanZheng-Regular.ttf",
    "NotoSerifSC.ttf": f"{GF}/notoserifsc/NotoSerifSC%5Bwght%5D.ttf",
}


def font(name):
    path = FONT_DIR / name
    if not path.exists():
        FONT_DIR.mkdir(parents=True, exist_ok=True)
        print(f"downloading {name}")
        urllib.request.urlretrieve(FONTS[name], path)
    return path


def cloud_mask(path):
    """Black cloud on white: white pixels are blocked."""
    im = Image.new("RGB", (W, H), "white")
    d = ImageDraw.Draw(im)
    for cx, cy, r in [(175, 235, 105), (300, 175, 140), (430, 230, 115)]:
        d.ellipse((cx - r, cy - r, cx + r, cy + r), fill="black")
    d.rounded_rectangle((90, 230, 520, 345), radius=58, fill="black")
    im.save(path)


def heart(mask_path, colors_path):
    """Black heart mask, plus colour bands that the words take their colours from."""
    shape = Image.new("RGB", (W, H), "white")
    d = ImageDraw.Draw(shape)
    d.ellipse((115, 10, 305, 200), fill="black")
    d.ellipse((295, 10, 485, 200), fill="black")
    d.polygon([(124, 140), (476, 140), (300, 392)], fill="black")
    shape.save(mask_path)
    bands = ["#e63946", "#f4a261", "#e9c46a", "#2a9d8f", "#457b9d", "#7b2cbf"]
    grad = Image.new("RGB", (W, H))
    d = ImageDraw.Draw(grad)
    step = H / len(bands)
    for i, c in enumerate(bands):
        d.rectangle((0, int(i * step), W, int((i + 1) * step)), fill=c)
    grad.save(colors_path)


def shrink(path):
    """Palette-compresses a rendered PNG (word clouds use few colours)."""
    im = Image.open(path)
    if im.mode == "RGBA":
        im = im.quantize(256, method=Image.Quantize.FASTOCTREE)
    else:
        im = im.convert("RGB").quantize(256)
    im.save(path, optimize=True)


def run(*args):
    out = args[args.index("--imagefile") + 1]
    subprocess.run([str(BIN), *map(str, args)], check=True)
    shrink(out)


def english(common):
    alice = GOLDEN / "alice.txt"
    zh = SRC / "zh_rust.txt"
    zh.write_text(zh_words.weighted_text(zh_words.RUST), encoding="utf-8")

    # Shape mask + contour
    run("--text", alice, "--mask", SRC / "cloud_mask.png", "--contour_width", 3,
        "--contour_color", "#4a7bb7", "--background", "white", "--colormap", "winter",
        "--random_state", 3, "--imagefile", OUT / "mask.png")
    # Colours taken from an image
    run("--text", alice, "--mask", SRC / "heart_mask.png", "--colormask", SRC / "rainbow.png",
        "--background", "white", "--max_words", 400, "--random_state", 3,
        "--imagefile", OUT / "image_colors.png")
    # Colormaps
    run("--text", GOLDEN / "a_new_hope.txt", *common, "--colormap", "plasma",
        "--imagefile", OUT / "colormap.png")
    # CJK text with a custom font
    run("--text", zh, *common, "--fontfile", font("LXGWWenKai-Medium.ttf"),
        "--background", "white", "--colormap", "viridis", "--no_collocations",
        "--relative_scaling", 0.5, "--imagefile", OUT / "cjk.png")
    # Single colour, horizontal only
    run("--text", GOLDEN / "constitution.txt", *common, "--color", "#b7410e",
        "--background", "white", "--prefer_horizontal", 1, "--relative_scaling", 0.6,
        "--imagefile", OUT / "single_color.png")
    # Transparent background
    run("--text", alice, *common, "--mode", "RGBA", "--background", "None",
        "--colormap", "cool", "--imagefile", OUT / "transparent.png")


def chinese(common):
    out = OUT / "zh"
    out.mkdir(exist_ok=True)
    texts = {}
    for name in ["WEATHER", "LOVE", "JOURNEY", "POEMS", "RUST", "FOOD"]:
        texts[name] = SRC / f"zh_{name.lower()}.txt"
        texts[name].write_text(zh_words.weighted_text(getattr(zh_words, name)), encoding="utf-8")
    zh = [*common, "--no_collocations", "--relative_scaling", 0.5]
    wenkai = font("LXGWWenKai-Medium.ttf")

    run("--text", texts["WEATHER"], *zh, "--fontfile", wenkai,
        "--mask", SRC / "cloud_mask.png", "--contour_width", 3, "--contour_color", "#4a7bb7",
        "--background", "white", "--colormap", "winter", "--imagefile", out / "mask.png")
    run("--text", texts["LOVE"], *zh, "--fontfile", font("ZCOOLKuaiLe-Regular.ttf"),
        "--mask", SRC / "heart_mask.png", "--colormask", SRC / "rainbow.png",
        "--background", "white", "--imagefile", out / "image_colors.png")
    run("--text", texts["JOURNEY"], *zh, "--fontfile", font("NotoSerifSC.ttf"),
        "--colormap", "plasma", "--imagefile", out / "colormap.png")
    run("--text", texts["POEMS"], *zh, "--fontfile", font("MaShanZheng-Regular.ttf"),
        "--background", "#1c1c1c", "--colormap", "autumn", "--imagefile", out / "font.png")
    run("--text", texts["RUST"], *zh, "--fontfile", wenkai, "--color", "#b7410e",
        "--background", "white", "--prefer_horizontal", 1, "--imagefile", out / "single_color.png")
    run("--text", texts["FOOD"], *zh, "--fontfile", wenkai, "--mode", "RGBA",
        "--background", "None", "--colormap", "cool", "--imagefile", out / "transparent.png")


def main():
    SRC.mkdir(exist_ok=True)
    cloud_mask(SRC / "cloud_mask.png")
    heart(SRC / "heart_mask.png", SRC / "rainbow.png")
    common = ["--random_state", 3, "--width", W, "--height", H]
    english(common)
    chinese(common)


if __name__ == "__main__":
    main()

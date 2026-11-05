"""Run the legacy Python word cloud with Pillow instrumented.

Usage: record_pillow.py OUT_PREFIX -- <wordcloud_cli args...>

Writes:
  OUT_PREFIX.glyphs.json  every TransposedFont bbox/mask Pillow produced (for ReplayRasterizer)
  OUT_PREFIX.layout.json  WordCloud.layout_ in the Rust --dump-layout schema
  OUT_PREFIX.png          the legacy rendered image
"""
import json
import sys

from PIL import ImageDraw, ImageFont

from wordcloud import wordcloud_cli
import wordcloud as wc_mod

BBOX, MASK = {}, {}


def key(font, text):
    return "%d|%d|%s" % (font.font.size, 0 if font.orientation is None else 1, text)


orig_textbbox = ImageDraw.ImageDraw.textbbox
orig_getbbox = ImageFont.TransposedFont.getbbox
orig_getmask = ImageFont.TransposedFont.getmask


def textbbox(self, xy, text, font=None, *a, **k):
    r = orig_textbbox(self, xy, text, font, *a, **k)
    if isinstance(font, ImageFont.TransposedFont):
        val = [int(r[2]), int(r[3])]
        assert r[2] == val[0] and r[3] == val[1] and r[0] == 0 and r[1] == 0, r
        assert BBOX.setdefault(key(font, text), val) == val
    return r


def getbbox(self, text, *a, **k):
    r = orig_getbbox(self, text, *a, **k)
    val = [int(r[2]), int(r[3])]
    assert BBOX.setdefault(key(self, text), val) == val, (key(self, text), val)
    return r


def getmask(self, text, mode="", *a, **k):
    im = orig_getmask(self, text, mode, *a, **k)
    assert mode == "L", mode
    from PIL import Image
    data = Image.Image()._new(im).tobytes()
    MASK[key(self, text)] = [im.size[0], im.size[1], data.hex()]
    return im


ImageDraw.ImageDraw.textbbox = textbbox
ImageFont.TransposedFont.getbbox = getbbox
ImageFont.TransposedFont.getmask = getmask


def main():
    prefix = sys.argv[1]
    assert sys.argv[2] == "--"
    args, text, imagefile = wordcloud_cli.parse_args(sys.argv[3:])
    imagefile.close()
    wc = wc_mod.WordCloud(**args)
    wc.generate(text)
    img = wc.to_image()
    img.save(prefix + ".png")
    layout = [
        {
            "word": w, "freq": f, "font_size": int(fs), "position": [int(p[0]), int(p[1])],
            "orientation": "Horizontal" if o is None else "Vertical", "color": c,
        }
        for (w, f), fs, p, o, c in wc.layout_
    ]
    with open(prefix + ".layout.json", "w") as fh:
        json.dump(layout, fh)
    with open(prefix + ".glyphs.json", "w") as fh:
        json.dump({"bbox": BBOX, "mask": MASK}, fh)


if __name__ == "__main__":
    main()

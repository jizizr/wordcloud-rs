# wordcloud (Rust)

[![crates.io](https://img.shields.io/crates/v/wordcloud.svg)](https://crates.io/crates/wordcloud)
[![docs.rs](https://docs.rs/wordcloud/badge.svg)](https://docs.rs/wordcloud)

A Rust port of [amueller/word_cloud](https://github.com/amueller/word_cloud) (Python + Cython, v1.9.6).

> **Package name:** this repository is `wordcloud-rs`, but the crate is published
> on crates.io as **[`wordcloud`](https://crates.io/crates/wordcloud)**, the same
> name as `pip install wordcloud`. The crates.io crate called `wordcloud-rs` is an
> unrelated project. Install with `cargo add wordcloud` and import with
> `use wordcloud::...`.

* **Seed-exact**: CPython's Mersenne Twister, Python dict ordering, Pillow's
  `BLEND` and every legacy layout quirk are reproduced, so with the same glyph
  masks the layout and the PNG pixels match the Python package byte for byte.
* **Fast**: ~10× faster end to end than the Python/Cython original.
* **Safe**: `#![deny(unsafe_code)]`. The one exception is a single audited block
  (bounds-check elision in the occupancy window counter, guarded by an `assert!`),
  kept only because benchmarks show it is 12–15% faster than the safe forms.

## Install

```bash
cargo install wordcloud      # CLI, installs the `wordcloud` binary
cargo add wordcloud          # library
```

## CLI

Same flags and defaults as Python's `wordcloud_cli`. Run `wordcloud --help` for the full list.

```bash
# Text file in, PNG out, fixed seed for a reproducible layout
wordcloud --text alice.txt --imagefile alice.png --random_state 1

# Read from stdin, write to stdout
cat alice.txt | wordcloud > alice.png

# Larger canvas, white background, a colormap, and an SVG next to the PNG
wordcloud --text alice.txt --imagefile alice.png --svgfile alice.svg \
  --width 800 --height 400 --background white --colormap plasma

# Fill a shape: white pixels in the mask stay empty
wordcloud --text alice.txt --mask alice_mask.png --contour_width 3 \
  --contour_color steelblue --imagefile alice.png
```

Extras over Python: `--colormap` actually works (Python silently ignores it),
`--svgfile out.svg`, and `--fix-legacy-bugs`.

## Library

Minimal example: text in, PNG out.

```rust
use std::fs::{self, File};
use wordcloud::{render, WordCloud};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let text = fs::read_to_string("alice.txt")?;

    let wc = WordCloud::builder()
        .size(800, 400)
        .max_words(300)
        .seed(1) // same seed, same layout
        .build()?;

    let layout = wc.generate(&text)?;
    render::write_png(&render::to_image(&wc, &layout)?, File::create("alice.png")?)?;
    Ok(())
}
```

Your own frequencies, colours and SVG output:

```rust
use wordcloud::{color::ColorFunc, svg, Config, WordCloud};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let freqs = vec![
        ("Rust".to_string(), 10.0),
        ("safe".to_string(), 6.0),
        ("fast".to_string(), 5.0),
        ("cargo".to_string(), 3.0),
    ];

    let config = Config {
        width: 600,
        height: 300,
        background_color: Some("white".into()),
        ..Config::default()
    };
    let wc = WordCloud::builder()
        .config(config)
        .color_func(ColorFunc::colormap("plasma")?) // or ColorFunc::single("darkblue")?
        .seed(7)
        .build()?;

    let layout = wc.generate_from_frequencies(&freqs)?;
    for w in &layout.words {
        println!("{} size={} at {:?}", w.word, w.font_size, w.position);
    }
    std::fs::write("cloud.svg", svg::to_svg(&wc, &layout)?)?;
    Ok(())
}
```

Word shapes come from a `Mask`. White pixels are blocked, and the canvas takes
the mask's size. This example loads the mask with the
[`image`](https://crates.io/crates/image) crate (`cargo add image`):

```rust
use std::fs::{self, File};
use wordcloud::{render, Mask, WordCloud};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let img = image::open("alice_mask.png")?.to_rgb8();
    let mask = Mask::from_pixels(img.width() as usize, img.height() as usize, 3, img.as_raw())?;

    let wc = WordCloud::builder().mask(mask).seed(1).build()?;
    let layout = wc.generate(&fs::read_to_string("alice.txt")?)?;
    render::write_png(&render::to_image(&wc, &layout)?, File::create("alice.png")?)?;
    Ok(())
}
```

Stopwords, collocations, the tokeniser regex and the minimum word length can be
set through `Config::text`. See the [API docs](https://docs.rs/wordcloud) for every option.

## Font rendering and parity

The default rasteriser is pure Rust (`ab_glyph`). FreeType + HarfBuzz hinting
differs slightly, so the *exact* layout for a seed differs from Python. To prove
the rest of the engine matches, `tools/record_pillow.py` records every glyph
Pillow produced and the hidden `--replay-glyphs` flag feeds them to the Rust
engine. `tools/parity.py` then compares layouts and pixels.

## Known differences

* Palette-mode (`P`) mask images are decoded to colours (Python compares the
  palette *indices* to 255).
* Grey-scale colour masks are accepted (Python raises `NotImplementedError`).
* `--contour_width` with `--mode RGBA` works (Python crashes on a broadcast error).
* Python's `\w` is approximated by `[\p{L}\p{N}_]`.
* SVG font embedding (`embed_font`) is not implemented.
* Contour rendering with `scale != 1` uses Catmull-Rom resizing, which approximates Pillow's bicubic filter.

## Repository layout

| Path | Contents |
|---|---|
| `src/` | Library and `wordcloud` binary |
| `tests/` | Golden-data parity, property, CLI and engine tests (`tests/golden/` holds data generated from the Python package) |
| `tools/` | `record_pillow.py` + `parity.py` (A/B harness), `make_golden.py`, `gen_tables.py`, `bench.py` |
| `docs/` | Migration blueprint, telemetry and the interactive report (`migration_infographic.html`) |

## License

MIT, like the upstream project. See `LICENSE` and `assets/README.md` for the bundled font.

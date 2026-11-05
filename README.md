# wordcloud-rs

A Rust port of [amueller/word_cloud](https://github.com/amueller/word_cloud) (Python + Cython, v1.9.6).

* **Seed-exact**: CPython's Mersenne Twister, Python dict ordering, Pillow's
  `BLEND` and every legacy layout quirk are reproduced, so with the same glyph
  masks the layout and the PNG pixels match the Python package byte for byte.
* **Fast**: ~10× faster end to end than the Python/Cython original.
* **Safe**: `#![deny(unsafe_code)]`. The one exception is a single audited block
  (bounds-check elision in the occupancy window counter, guarded by an `assert!`),
  kept only because benchmarks show it is 12–15% faster than the safe forms.

## CLI

Same flags and defaults as `wordcloud_cli`:

```bash
cargo run --release -- --text alice.txt --imagefile alice.png --random_state 1
```

Extras: `--colormap` actually works (it is silently ignored in Python),
`--svgfile out.svg`, `--fix-legacy-bugs`.

## Library

```rust
use wordcloud_rs::{render, WordCloud};
let wc = WordCloud::builder().size(800, 400).max_words(300).seed(1).build()?;
let layout = wc.generate(&std::fs::read_to_string("alice.txt")?)?;
render::write_png(&render::to_image(&wc, &layout)?, std::fs::File::create("alice.png")?)?;
```

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

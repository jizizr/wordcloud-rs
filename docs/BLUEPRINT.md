# word_cloud → Safe Rust: Architectural Blueprint

Source: `amueller/word_cloud` @ `93c5e4c` (wordcloud 1.9.6) — Python 3 + one Cython kernel.
Target: `wordcloud-rs/`, a `#![forbid(unsafe_code)]` library + `wordcloud` CLI binary.

## 1. Codebase Audit, Legacy Census & Latent Bugs

**Census (telemetry `before` scan of `wordcloud/`)**: 6 Python files + 1 `.pyx`, 1,202 code LOC,
9.5% comments, largest file 891 LOC (`wordcloud.py`, 83% of files ≤ 400 LOC), max nesting 13,
avg function 30 LOC, 7 memory/state hazards (mutable module globals, bounds-check-disabled Cython).
Legacy tests: 75/76 pass (the 1 failure is environmental: `wordcloud_cli` not on `PATH`).

**Runtime dependencies**: NumPy (integral image), Pillow + FreeType 2.14.3 + **raqm/HarfBuzz**
(glyph rasterisation, PNG), matplotlib (colormaps, also imported at start-up: ~300 ms).

**Latent bugs found (all verified or traced)**

| # | Location | Bug |
|---|----------|-----|
| 1 | `query_integral_image.pyx:26` | `randint(0, hits)` is inclusive; `goal == 0` never matches → returns `None` with free space (38/2000 seeds on an empty canvas). |
| 2 | `wordcloud.py:523` | `ROTATE_90 if orientation is None else ROTATE_90` — retry never flips a vertical word back to horizontal. |
| 3 | `wordcloud.py:549` | `uint8 + bool` wraps 255 → 0: ink on a masked pixel becomes "free". |
| 4 | `wordcloud_cli.py:244` | `color_func` is always set, so `--colormap` is silently ignored. |
| 5 | `color_from_image.py:47` | Patch sliced with width on the row axis and height on the column axis. |
| 6 | `wordcloud.py:323` | matplotlib version compared as strings: 10.x would select `hsv`. |
| 7 | `wordcloud.py:534` | Occupancy window is rows `(i, i+size]`, ink starts at `i + margin//2`; with margin < 2 the first ink row/col is unchecked. |

**Port policy**: the Rust engine reproduces bugs 1, 2, 3, 5 and 7 *by default*
(`legacy_quirks = true`), because they change RNG consumption and layout and
byte-for-byte parity is the verification oracle. `legacy_quirks = false` (CLI `--fix-legacy-bugs`)
fixes 1 and 2. Bug 4 is fixed in the Rust CLI (`--colormap` works when `--color`/`--colormask` are
absent). Bug 6 does not apply.

## 2. Target Safe Rust Architecture & Module Graph

```
wordcloud-rs/src
├── lib.rs            crate docs, re-exports, #![forbid(unsafe_code)]
├── main.rs           CLI entry, #![forbid(unsafe_code)]
├── cli.rs            clap argument model mirroring wordcloud_cli.py
├── rng.rs            PyRandom: CPython-exact MT19937 (seed, random, randint, uniform)
├── text/{mod,tokenize,counts}.rs   process_text, process_tokens, unigrams_and_bigrams, Dunning score
├── occupancy.rs      flat SoA integral image (u32, wrapping) + query kernel
├── raster/{mod,ab,replay}.rs       TextRasterizer trait; ab_glyph backend; Pillow replay backend
├── canvas.rs         Gray/RGB canvases with Pillow-exact BLEND/DIV255 alpha blending
├── layout.rs         WordCloud config + generate_from_frequencies → Layout
├── color/{mod,names,colormaps}.rs  colour parsing, colour functions, matplotlib LUTs
├── image_color.rs    ImageColorGenerator
├── render.rs         to_image (scale, contour), PNG save
└── svg.rs            to_svg
```

| Legacy symbol (Python/Cython) | Rust module & type/fn | Transformation |
|---|---|---|
| `random.Random` (implicit dependency) | `rng.rs::PyRandom` | Bit-exact MT19937 + CPython seeding/`_randbelow` |
| `query_integral_image.pyx::query_integral_image` | `occupancy.rs::OccupancyMap::sample_position` | Bounds-checked slices, wrapping `u32`, no NumPy |
| `wordcloud.py::IntegralOccupancyMap` | `occupancy.rs::OccupancyMap` | In-place row-major update, zero allocation |
| `tokenization.py::{score,l,process_tokens,unigrams_and_bigrams}` | `text/counts.rs` | `IndexMap` keeps Python dict insertion order |
| `WordCloud.process_text` | `text/tokenize.rs::process_text` | Python `\w` ≈ `[\p{L}\p{N}_]` |
| `WordCloud.generate_from_frequencies` | `layout.rs::WordCloud::generate_from_frequencies` | Recursive probe → explicit `probe_font_size()` |
| Pillow `textbbox`/`getmask2`/`TransposedFont` | `raster/*::TextRasterizer` | Trait; rotation = CCW transpose of mask |
| Pillow `ImageDraw.text` → `fill_mask_L` | `canvas.rs::blend_mask` | Exact `BLEND`/`DIV255` integer formula |
| `random_color_func`, `colormap_color_func`, `get_single_color_func` | `color/mod.rs::ColorFunc` | Enum, no closures over globals |
| `ImageColor.getrgb`, matplotlib LUTs | `color/{names,colormaps}.rs` | Generated constant tables |
| `to_image`, `_draw_contour`, `to_file` | `render.rs` | `image` crate PNG |
| `to_svg` | `svg.rs` | Font embedding (fontTools) out of scope |
| `color_from_image.py::ImageColorGenerator` | `image_color.rs` | Same (quirky) patch semantics |
| `wordcloud_cli.py` | `cli.rs` + `main.rs` | clap; same flags/defaults |

Every file targets ≤ 400 LOC; generated tables are one line per entry.

## 3. API Ergonomics & Call-Site Modernisation

Before (Python):
```python
wc = WordCloud(width=1000, height=700, max_words=400, random_state=1)
wc.generate(open("alice.txt").read())
wc.to_file("alice.png")
```
After (Rust):
```rust
let cloud = WordCloud::builder().size(1000, 700).max_words(400).seed(1).build()?;
let layout = cloud.generate(&std::fs::read_to_string("alice.txt")?)?;
render::save_png(&cloud, &layout, "alice.png")?;
```
`Layout` is an immutable value returned from `generate` instead of `layout_`/`words_` attributes
mutated on `self`; errors are `Result<_, WordCloudError>` instead of `ValueError`/`IndexError`.

## 4. Test Harness, Property Fuzzing & Parity Strategy

* **Unit + doc tests** for every module; **proptest** for PRNG range invariants, occupancy
  (sampled windows are always empty), tokenizer invariants (no stopwords, counts > 0), colour parsing.
* **A/B parity (three tiers)**, golden data produced by the legacy Python code:
  1. *PRNG*: `random()`, `randint`, `uniform`, `getrandbits` streams vs CPython for N seeds — exact.
  2. *Text*: `process_text` word→count maps (ordered) for several corpora × option sets — exact.
  3. *Full pipeline*: Python run instrumented to record every Pillow glyph bbox/mask; the Rust
     `ReplayRasterizer` feeds the identical masks into the Rust engine, and the resulting layout
     (words, sizes, positions, orientations, colours) **and final RGB pixels** are compared for
     N seeds. Epsilon: 0 (integer outputs); float colour channels compared via formatted strings.
* The pure-Rust `ab_glyph` backend cannot match FreeType+HarfBuzz hinting bit-for-bit; its output is
  validated visually and structurally (same word set ordering, plausible sizes), and documented.

## 5. 4-Stage Benchmarking & PGO Pipeline Plan

Workload: `alice.txt`, 1000×700, `max_words=400`, seeds 0–4, end-to-end CLI wall-clock + peak RSS.
1. `legacy_baseline` — Python/Cython: **3519 ms, 83.7 MB** (measured).
2. `safe_rust` — direct port: per-placement integral recompute into a fresh buffer, re-rasterise glyphs.
3. `soa_simd` — flat in-place integral update, glyph-mask cache keyed by (word, size, orientation),
   row-slice query loops shaped for auto-vectorisation.
4. `pgo_lto` — `-Cprofile-generate` → `llvm-profdata merge` → `-Cprofile-use`, fat LTO, 1 CGU.

## 6. Phased Execution Roadmap

1. Scaffold crate, `PyRandom` + parity tests vs CPython.
2. Text pipeline + parity tests.
3. Occupancy, canvas blend, rasterisers, layout engine, colours, render, SVG, CLI.
4. Python recorder (`tools/record_pillow.py`) + replay parity across N seeds.
5. Test crucible (unit/doc/proptest, coverage via `cargo llvm-cov` if available), clippy clean.
6. Staircase benchmarks, PGO, final census, infographic.

**Open questions**: none blocking. Font embedding in SVG and CTL (raqm) shaping for complex
scripts are out of scope for the pure-Rust rasteriser.

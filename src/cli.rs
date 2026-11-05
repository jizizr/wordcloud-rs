//! Command-line interface mirroring `wordcloud_cli.py` (same flags and defaults).
//!
//! Differences from the legacy CLI:
//! * `--colormap` works (legacy bug #4: it was always ignored); without it the
//!   legacy default `random_color_func` is used, so default output is unchanged;
//! * `--fix-legacy-bugs` disables the reproduced layout quirks;
//! * `--svgfile` additionally writes an SVG;
//! * hidden `--replay-glyphs`/`--dump-layout` power the A/B parity harness.

use crate::color::ColorFunc;
use crate::config::{Config, Mask};
use crate::error::{Result, WordCloudError};
use crate::image_color::ImageColorGenerator;
use crate::layout::WordCloud;
use crate::raster::{AbGlyphRasterizer, ReplayRasterizer, TextRasterizer};
use crate::text::parse_stopwords;
use crate::{render, svg};
use clap::Parser;
use std::io::{Read, Write};
use std::path::PathBuf;

/// Glyph-mask caching is disabled in the `naive` benchmarking build.
const CACHE: bool = !cfg!(feature = "naive");

/// A simple command line interface for wordcloud module.
#[derive(Parser, Debug)]
#[command(name = "wordcloud", version)]
pub struct Args {
    /// specify file of words to build the word cloud (default: stdin)
    #[arg(long, default_value = "-")]
    pub text: String,
    /// override the regular expression defining what constitutes a word
    #[arg(long)]
    pub regexp: Option<String>,
    /// specify file of stopwords (containing one word per line) to remove from the given text after parsing
    #[arg(long)]
    pub stopwords: Option<PathBuf>,
    /// file the completed PNG image should be written to (default: stdout)
    #[arg(long, default_value = "-")]
    pub imagefile: String,
    /// path to font file you wish to use (default: DroidSansMono)
    #[arg(long)]
    pub fontfile: Option<PathBuf>,
    /// mask to use for the image form
    #[arg(long)]
    pub mask: Option<PathBuf>,
    /// color mask to use for image coloring
    #[arg(long)]
    pub colormask: Option<PathBuf>,
    /// if greater than 0, draw mask contour (default: 0)
    #[arg(long = "contour_width", default_value_t = 0.0)]
    pub contour_width: f64,
    /// use given color as mask contour color
    #[arg(long = "contour_color", default_value = "black")]
    pub contour_color: String,
    /// scaling of words by frequency (0 - 1)
    #[arg(long = "relative_scaling", default_value_t = 0.0)]
    pub relative_scaling: f64,
    /// spacing to leave around words
    #[arg(long, default_value_t = 2)]
    pub margin: usize,
    /// define output image width
    #[arg(long, default_value_t = 400)]
    pub width: usize,
    /// define output image height
    #[arg(long, default_value_t = 200)]
    pub height: usize,
    /// use given color as coloring for the image - accepts any value from PIL.ImageColor.getcolor
    #[arg(long)]
    pub color: Option<String>,
    /// use given color as background color for the image
    #[arg(long, default_value = "black")]
    pub background: String,
    /// do not add collocations (bigrams) to word cloud
    #[arg(long = "no_collocations")]
    pub no_collocations: bool,
    /// include numbers in wordcloud?
    #[arg(long = "include_numbers")]
    pub include_numbers: bool,
    /// only include words with more than X letters
    #[arg(long = "min_word_length", default_value_t = 0)]
    pub min_word_length: usize,
    /// ratio of times to try horizontal fitting as opposed to vertical
    #[arg(long = "prefer_horizontal", default_value_t = 0.9)]
    pub prefer_horizontal: f64,
    /// scaling between computation and drawing
    #[arg(long, default_value_t = 1.0)]
    pub scale: f64,
    /// matplotlib colormap name
    #[arg(long)]
    pub colormap: Option<String>,
    /// use RGB or RGBA for transparent background
    #[arg(long, default_value = "RGB")]
    pub mode: String,
    /// maximum number of words
    #[arg(long = "max_words", default_value_t = 200)]
    pub max_words: usize,
    /// smallest font size to use
    #[arg(long = "min_font_size", default_value_t = 4)]
    pub min_font_size: i64,
    /// maximum font size for the largest word
    #[arg(long = "max_font_size")]
    pub max_font_size: Option<i64>,
    /// step size for the font
    #[arg(long = "font_step", default_value_t = 1)]
    pub font_step: i64,
    /// random seed
    #[arg(long = "random_state")]
    pub random_state: Option<i64>,
    /// whether to remove trailing 's' from words
    #[arg(long = "no_normalize_plurals")]
    pub no_normalize_plurals: bool,
    /// whether to repeat words and phrases
    #[arg(long)]
    pub repeat: bool,
    /// fix legacy layout quirks (changes output for a given seed)
    #[arg(long = "fix-legacy-bugs")]
    pub fix_legacy_bugs: bool,
    /// also write an SVG rendering to this file
    #[arg(long)]
    pub svgfile: Option<PathBuf>,
    /// replay glyph masks recorded from Pillow (parity testing)
    #[arg(long = "replay-glyphs", hide = true)]
    pub replay_glyphs: Option<PathBuf>,
    /// write the layout as JSON (parity testing)
    #[arg(long = "dump-layout", hide = true)]
    pub dump_layout: Option<PathBuf>,
}

fn load_pixels(path: &PathBuf) -> Result<(usize, usize, usize, Vec<u8>)> {
    let img = image::open(path)?;
    let (w, h) = (img.width() as usize, img.height() as usize);
    Ok(match img {
        image::DynamicImage::ImageLuma8(i) => (w, h, 1, i.into_raw()),
        image::DynamicImage::ImageLumaA8(i) => (w, h, 2, i.into_raw()),
        image::DynamicImage::ImageRgb8(i) => (w, h, 3, i.into_raw()),
        other => (w, h, 4, other.to_rgba8().into_raw()),
    })
}

fn color_func(args: &Args) -> Result<ColorFunc> {
    if args.colormask.is_some() && args.color.is_some() {
        return Err(WordCloudError::InvalidConfig(
            "specify either a color mask or a color function".into(),
        ));
    }
    if let Some(c) = &args.color {
        return ColorFunc::single(c);
    }
    if let Some(path) = &args.colormask {
        let rgb = image::open(path)?.to_rgb8();
        let (w, h) = (rgb.width() as usize, rgb.height() as usize);
        return Ok(ColorFunc::Image(Box::new(ImageColorGenerator::new(
            w,
            h,
            rgb.into_raw(),
            None,
        )?)));
    }
    match &args.colormap {
        Some(name) => ColorFunc::colormap(name),
        None => Ok(ColorFunc::RandomHue),
    }
}

/// Translates parsed arguments into a configured [`WordCloud`].
///
/// # Errors
/// Invalid files, colours or configuration.
pub fn build(args: &Args) -> Result<WordCloud> {
    let mut cfg = Config {
        width: args.width,
        height: args.height,
        margin: args.margin,
        prefer_horizontal: args.prefer_horizontal,
        scale: args.scale,
        max_words: args.max_words,
        min_font_size: args.min_font_size,
        max_font_size: args.max_font_size,
        font_step: args.font_step,
        relative_scaling: args.relative_scaling,
        repeat: args.repeat,
        seed: args.random_state,
        background_color: (args.background != "None").then(|| args.background.clone()),
        rgba: args.mode.eq_ignore_ascii_case("RGBA"),
        contour_width: args.contour_width,
        contour_color: args.contour_color.clone(),
        legacy_quirks: !args.fix_legacy_bugs,
        ..Config::default()
    };
    cfg.text.regexp = args.regexp.clone();
    cfg.text.collocations = !args.no_collocations;
    cfg.text.normalize_plurals = !args.no_normalize_plurals;
    cfg.text.include_numbers = args.include_numbers;
    cfg.text.min_word_length = args.min_word_length;
    if let Some(p) = &args.stopwords {
        cfg.text.stopwords = parse_stopwords(&std::fs::read_to_string(p)?);
    }
    let raster: Box<dyn TextRasterizer> = match (&args.replay_glyphs, &args.fontfile) {
        (Some(rec), _) => Box::new(ReplayRasterizer::from_json(&std::fs::read_to_string(rec)?)?),
        (None, Some(font)) => Box::new(AbGlyphRasterizer::from_bytes(std::fs::read(font)?, CACHE)?),
        (None, None) => Box::new(AbGlyphRasterizer::default_font(CACHE)?),
    };
    let mut builder = WordCloud::builder()
        .config(cfg)
        .rasterizer(raster)
        .color_func(color_func(args)?);
    if let Some(p) = &args.mask {
        let (w, h, ch, px) = load_pixels(p)?;
        builder = builder.mask(Mask::from_pixels(w, h, ch, &px)?);
    }
    builder.build()
}

/// Runs the CLI end to end.
///
/// # Errors
/// Any I/O, parsing, layout or rendering error.
pub fn run(args: &Args) -> Result<()> {
    let wc = build(args)?;
    let text = if args.text == "-" {
        let mut s = String::new();
        std::io::stdin().read_to_string(&mut s)?;
        s
    } else {
        std::fs::read_to_string(&args.text)?
    };
    let layout = wc.generate(&text)?;
    if let Some(p) = &args.dump_layout {
        std::fs::write(p, serde_json::to_string(&layout.words)?)?;
    }
    if let Some(p) = &args.svgfile {
        std::fs::write(p, svg::to_svg(&wc, &layout)?)?;
    }
    let img = render::to_image(&wc, &layout)?;
    if args.imagefile == "-" {
        let mut out = std::io::stdout().lock();
        render::write_png(&img, &mut out)?;
        out.flush()?;
    } else {
        render::write_png(
            &img,
            std::io::BufWriter::new(std::fs::File::create(&args.imagefile)?),
        )?;
    }
    Ok(())
}

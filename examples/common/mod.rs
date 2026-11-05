//! Helpers shared by the gallery examples: input texts, Chinese keyword
//! extraction, fonts and compact PNG output. The word-cloud code itself lives
//! in each example.
#![allow(dead_code)]

use std::error::Error;
use std::path::{Path, PathBuf};

use jieba_rs::{Jieba, KeywordExtract, TfIdf};
use wordcloud::canvas::Canvas;
use wordcloud::raster::{AbGlyphRasterizer, TextRasterizer};

pub type AnyResult<T = ()> = Result<T, Box<dyn Error>>;

/// Size of every gallery image.
pub const WIDTH: usize = 600;
pub const HEIGHT: usize = 400;

/// Seed used for every gallery image, so `cargo run --example ...` is reproducible.
pub const SEED: i64 = 3;

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

/// Reads a file relative to the repository root.
pub fn read(path: &str) -> AnyResult<String> {
    Ok(std::fs::read_to_string(root().join(path))?)
}

/// Keywords and weights of a Chinese text, ready for `generate_from_frequencies`.
///
/// The bundled classics are in traditional characters; they are converted to
/// simplified (`fast2s`), segmented and scored with TF-IDF (`jieba-rs`).
/// Only nouns, names, places, idioms and adjectives are kept, so function words
/// such as 不知 or 只见 do not crowd out the content.
pub fn chinese_keywords(path: &str, top_k: usize) -> AnyResult<Vec<(String, f64)>> {
    let text = fast2s::convert(&read(path)?);
    let jieba = Jieba::new();
    let pos = [
        "n", "nr", "nrt", "nrfg", "ns", "nt", "nz", "vn", "an", "a", "i", "l",
    ];
    let keywords = TfIdf::default().extract_keywords(
        &jieba,
        &text,
        top_k,
        pos.iter().map(|p| p.to_string()).collect(),
    );
    Ok(keywords
        .into_iter()
        .filter(|k| k.keyword.chars().all(is_han))
        .map(|k| (k.keyword, k.weight))
        .collect())
}

fn is_han(c: char) -> bool {
    matches!(c, '\u{3400}'..='\u{4DBF}' | '\u{4E00}'..='\u{9FFF}' | '\u{F900}'..='\u{FAFF}')
}

/// Path of a font in `examples/data/fonts/` (see `examples/fetch_fonts.sh`).
pub fn font_path(file: &str) -> PathBuf {
    root().join("examples/data/fonts").join(file)
}

/// Loads a font from `examples/data/fonts/` (see `examples/fetch_fonts.sh`).
pub fn font(file: &str) -> AnyResult<Box<dyn TextRasterizer>> {
    let path = font_path(file);
    let bytes = std::fs::read(&path).map_err(|e| {
        format!(
            "cannot read {}: {e}. Run `sh examples/fetch_fonts.sh` first.",
            path.display()
        )
    })?;
    Ok(Box::new(AbGlyphRasterizer::from_bytes(bytes, true)?))
}

/// Output path for a gallery image: `docs/gallery/<name>` or `docs/gallery/zh/<name>`.
pub fn out(lang: &str, name: &str) -> PathBuf {
    let dir = match lang {
        "zh" => root().join("docs/gallery/zh"),
        _ => root().join("docs/gallery"),
    };
    std::fs::create_dir_all(&dir).expect("create output directory");
    dir.join(name)
}

/// Writes `img` as a 256-colour palette PNG (word clouds use few colours, so
/// this is ~4x smaller than the RGB(A) file with no visible difference).
pub fn save(img: &Canvas, path: &Path) -> AnyResult {
    let rgba: Vec<u8> = match img.channels {
        4 => img.data.clone(),
        3 => img
            .data
            .chunks_exact(3)
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
        n => return Err(format!("unsupported channel count {n}").into()),
    };
    let nq = color_quant::NeuQuant::new(10, 256, &rgba);
    let indices: Vec<u8> = rgba.chunks_exact(4).map(|p| nq.index_of(p) as u8).collect();
    let map = nq.color_map_rgba();

    let file = std::io::BufWriter::new(std::fs::File::create(path)?);
    let mut enc = png::Encoder::new(file, img.width as u32, img.height as u32);
    enc.set_color(png::ColorType::Indexed);
    enc.set_depth(png::BitDepth::Eight);
    enc.set_compression(png::Compression::High);
    enc.set_palette(
        map.chunks_exact(4)
            .flat_map(|c| [c[0], c[1], c[2]])
            .collect::<Vec<_>>(),
    );
    if img.channels == 4 {
        enc.set_trns(map.chunks_exact(4).map(|c| c[3]).collect::<Vec<_>>());
    }
    enc.write_header()?.write_image_data(&indices)?;
    println!("wrote {}", path.display());
    Ok(())
}

/// An RGB image buffer filled with white, for drawing masks.
pub struct Picture {
    pub width: usize,
    pub height: usize,
    pub rgb: Vec<u8>,
}

impl Picture {
    pub fn new(width: usize, height: usize, color: [u8; 3]) -> Self {
        Self {
            width,
            height,
            rgb: color.repeat(width * height),
        }
    }

    /// Paints every pixel for which `inside(x, y)` is true.
    pub fn fill(&mut self, color: [u8; 3], inside: impl Fn(f64, f64) -> bool) {
        for y in 0..self.height {
            for x in 0..self.width {
                if inside(x as f64, y as f64) {
                    let i = (y * self.width + x) * 3;
                    self.rgb[i..i + 3].copy_from_slice(&color);
                }
            }
        }
    }
}

//! Use any TTF/OTF/TTC font, for example to draw Chinese text, or calligraphy.
//!
//!     cargo run --release --example custom_font
//!
//! Writes docs/gallery/cjk.png and docs/gallery/zh/font.png.
//!
//! Chinese has no spaces between words, so the text is segmented first
//! (see `common::chinese_keywords`, which uses `jieba-rs`).

mod common;

use common::{AnyResult, HEIGHT, SEED, WIDTH};
use wordcloud::raster::AbGlyphRasterizer;
use wordcloud::{Config, WordCloud, color::ColorFunc, render};

fn main() -> AnyResult {
    // Journey to the West in LXGW WenKai, on white.
    let font = std::fs::read(common::font_path("LXGWWenKai-Medium.ttf"))?;
    let wc = WordCloud::builder()
        .config(Config {
            width: WIDTH,
            height: HEIGHT,
            background_color: Some("white".into()),
            ..Config::default()
        })
        .rasterizer(Box::new(AbGlyphRasterizer::from_bytes(font, true)?))
        .color_func(ColorFunc::colormap("viridis")?)
        .seed(SEED)
        .build()?;
    let freqs = common::chinese_keywords("examples/data/zh/xiyouji.txt", 200)?;
    let layout = wc.generate_from_frequencies(&freqs)?;
    common::save(
        &render::to_image(&wc, &layout)?,
        &common::out("en", "cjk.png"),
    )?;

    // The Book of Songs in a brush-script font, on near-black.
    let font = std::fs::read(common::font_path("MaShanZheng-Regular.ttf"))?;
    let wc = WordCloud::builder()
        .config(Config {
            width: WIDTH,
            height: HEIGHT,
            background_color: Some("#1c1c1c".into()),
            ..Config::default()
        })
        .rasterizer(Box::new(AbGlyphRasterizer::from_bytes(font, true)?))
        .color_func(ColorFunc::colormap("autumn")?)
        .seed(SEED)
        .build()?;
    let freqs = common::chinese_keywords("examples/data/zh/shijing.txt", 200)?;
    let layout = wc.generate_from_frequencies(&freqs)?;
    common::save(
        &render::to_image(&wc, &layout)?,
        &common::out("zh", "font.png"),
    )?;
    Ok(())
}

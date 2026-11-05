//! A transparent (RGBA) background, so the image can sit on any page colour.
//!
//!     cargo run --release --example transparent
//!
//! Writes docs/gallery/transparent.png and docs/gallery/zh/transparent.png.

mod common;

use common::{AnyResult, HEIGHT, SEED, WIDTH};
use wordcloud::{Config, WordCloud, color::ColorFunc, render};

fn builder() -> AnyResult<wordcloud::WordCloudBuilder> {
    let config = Config {
        width: WIDTH,
        height: HEIGHT,
        rgba: true,
        background_color: None, // no background: fully transparent
        ..Config::default()
    };
    Ok(WordCloud::builder()
        .config(config)
        .color_func(ColorFunc::colormap("cool")?)
        .seed(SEED))
}

fn main() -> AnyResult {
    let wc = builder()?.build()?;
    let layout = wc.generate(&common::read("tests/golden/alice.txt")?)?;
    common::save(
        &render::to_image(&wc, &layout)?,
        &common::out("en", "transparent.png"),
    )?;

    // Chinese: Strange Tales from a Chinese Studio (volume 1).
    let wc = builder()?
        .rasterizer(common::font("LXGWWenKai-Medium.ttf")?)
        .build()?;
    let freqs = common::chinese_keywords("examples/data/zh/liaozhai.txt", 200)?;
    let layout = wc.generate_from_frequencies(&freqs)?;
    common::save(
        &render::to_image(&wc, &layout)?,
        &common::out("zh", "transparent.png"),
    )?;
    Ok(())
}

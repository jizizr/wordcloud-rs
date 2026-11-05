//! Shades of a single colour, with every word laid out horizontally.
//!
//!     cargo run --release --example single_color
//!
//! Writes docs/gallery/single_color.png and docs/gallery/zh/single_color.png.

mod common;

use common::{AnyResult, HEIGHT, SEED, WIDTH};
use wordcloud::{Config, WordCloud, color::ColorFunc, render};

fn builder() -> AnyResult<wordcloud::WordCloudBuilder> {
    let config = Config {
        width: WIDTH,
        height: HEIGHT,
        background_color: Some("white".into()),
        prefer_horizontal: 1.0, // 1.0 = never rotate a word
        relative_scaling: 0.6,  // how strongly frequency drives font size
        ..Config::default()
    };
    Ok(WordCloud::builder()
        .config(config)
        .color_func(ColorFunc::single("#b7410e")?)
        .seed(SEED))
}

fn main() -> AnyResult {
    let wc = builder()?.build()?;
    let layout = wc.generate(&common::read("tests/golden/constitution.txt")?)?;
    common::save(
        &render::to_image(&wc, &layout)?,
        &common::out("en", "single_color.png"),
    )?;

    // Chinese: Water Margin (chapters 1-5).
    let wc = builder()?
        .rasterizer(common::font("LXGWWenKai-Medium.ttf")?)
        .build()?;
    let freqs = common::chinese_keywords("examples/data/zh/shuihu.txt", 200)?;
    let layout = wc.generate_from_frequencies(&freqs)?;
    common::save(
        &render::to_image(&wc, &layout)?,
        &common::out("zh", "single_color.png"),
    )?;
    Ok(())
}

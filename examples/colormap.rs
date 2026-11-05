//! Colour words with one of the 18 built-in matplotlib colormaps.
//!
//!     cargo run --release --example colormap
//!
//! Writes docs/gallery/colormap.png and docs/gallery/zh/colormap.png.

mod common;

use common::{AnyResult, HEIGHT, SEED, WIDTH};
use wordcloud::{WordCloud, color::ColorFunc, render};

fn builder() -> AnyResult<wordcloud::WordCloudBuilder> {
    Ok(WordCloud::builder()
        .size(WIDTH, HEIGHT)
        .color_func(ColorFunc::colormap("plasma")?) // viridis, magma, inferno, jet, ...
        .seed(SEED))
}

fn main() -> AnyResult {
    let wc = builder()?.build()?;
    let layout = wc.generate(&common::read("tests/golden/a_new_hope.txt")?)?;
    common::save(
        &render::to_image(&wc, &layout)?,
        &common::out("en", "colormap.png"),
    )?;

    // Chinese: Romance of the Three Kingdoms (chapters 1-5) in a serif font.
    let wc = builder()?
        .rasterizer(common::font("NotoSerifSC.ttf")?)
        .build()?;
    let freqs = common::chinese_keywords("examples/data/zh/sanguo.txt", 200)?;
    let layout = wc.generate_from_frequencies(&freqs)?;
    common::save(
        &render::to_image(&wc, &layout)?,
        &common::out("zh", "colormap.png"),
    )?;
    Ok(())
}

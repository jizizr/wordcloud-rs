//! Fill a shape: words are only placed where the mask is not white, and an
//! outline is drawn around the shape.
//!
//!     cargo run --release --example shape_mask
//!
//! Writes docs/gallery/mask.png and docs/gallery/zh/mask.png.

mod common;

use common::{AnyResult, HEIGHT, Picture, SEED, WIDTH};
use wordcloud::{Config, Mask, WordCloud, color::ColorFunc, render};

/// A cloud drawn in black on white. Normally you would load a picture instead:
/// `Mask::from_pixels(w, h, 3, image::open("shape.png")?.to_rgb8().as_raw())`.
fn cloud_mask() -> AnyResult<Mask> {
    let mut pic = Picture::new(WIDTH, HEIGHT, [255, 255, 255]);
    let circles = [
        (175.0, 235.0, 105.0),
        (300.0, 175.0, 140.0),
        (430.0, 230.0, 115.0),
    ];
    pic.fill([0, 0, 0], |x, y| {
        let in_circle = circles
            .iter()
            .any(|&(cx, cy, r)| (x - cx).powi(2) + (y - cy).powi(2) <= r * r);
        let in_base = (148.0..=462.0).contains(&x) && (230.0..=345.0).contains(&y);
        let in_ends = [(148.0, 287.5), (462.0, 287.5)]
            .iter()
            .any(|&(cx, cy)| (x - cx).powi(2) + (y - cy).powi(2) <= 57.5 * 57.5);
        in_circle || in_base || in_ends
    });
    Ok(Mask::from_pixels(pic.width, pic.height, 3, &pic.rgb)?)
}

fn builder() -> AnyResult<wordcloud::WordCloudBuilder> {
    let config = Config {
        background_color: Some("white".into()),
        contour_width: 3.0,
        contour_color: "#4a7bb7".into(),
        ..Config::default()
    };
    Ok(WordCloud::builder()
        .config(config)
        .mask(cloud_mask()?)
        .color_func(ColorFunc::colormap("winter")?)
        .seed(SEED))
}

fn main() -> AnyResult {
    // English: tokenise and count a novel.
    let wc = builder()?.build()?;
    let layout = wc.generate(&common::read("tests/golden/alice.txt")?)?;
    common::save(
        &render::to_image(&wc, &layout)?,
        &common::out("en", "mask.png"),
    )?;

    // Chinese: keywords of Journey to the West (chapters 1-5) with a Chinese font.
    let wc = builder()?
        .rasterizer(common::font("LXGWWenKai-Medium.ttf")?)
        .build()?;
    let freqs = common::chinese_keywords("examples/data/zh/xiyouji.txt", 200)?;
    let layout = wc.generate_from_frequencies(&freqs)?;
    common::save(
        &render::to_image(&wc, &layout)?,
        &common::out("zh", "mask.png"),
    )?;
    Ok(())
}

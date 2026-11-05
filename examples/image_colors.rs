//! Colour each word with the colours of a picture (`ImageColorGenerator`),
//! inside a heart-shaped mask.
//!
//!     cargo run --release --example image_colors
//!
//! Writes docs/gallery/image_colors.png and docs/gallery/zh/image_colors.png.

mod common;

use common::{AnyResult, HEIGHT, Picture, SEED, WIDTH};
use wordcloud::image_color::ImageColorGenerator;
use wordcloud::{Config, Mask, WordCloud, color::ColorFunc, render};

fn heart_mask() -> AnyResult<Mask> {
    let mut pic = Picture::new(WIDTH, HEIGHT, [255, 255, 255]);
    pic.fill([0, 0, 0], |x, y| {
        let lobe = |cx: f64| (x - cx).powi(2) + (y - 105.0).powi(2) <= 95.0 * 95.0;
        // Triangle (124,140) (476,140) (300,392).
        let tip = (140.0..=392.0).contains(&y) && (x - 300.0).abs() <= 176.0 * (392.0 - y) / 252.0;
        lobe(210.0) || lobe(390.0) || tip
    });
    Ok(Mask::from_pixels(pic.width, pic.height, 3, &pic.rgb)?)
}

/// The picture the colours come from: six horizontal bands. Any photo works.
fn rainbow() -> AnyResult<ColorFunc> {
    let bands = [
        [0xe6, 0x39, 0x46],
        [0xf4, 0xa2, 0x61],
        [0xe9, 0xc4, 0x6a],
        [0x2a, 0x9d, 0x8f],
        [0x45, 0x7b, 0x9d],
        [0x7b, 0x2c, 0xbf],
    ];
    let mut pic = Picture::new(WIDTH, HEIGHT, [255, 255, 255]);
    for (i, color) in bands.iter().enumerate() {
        let (top, bottom) = ((i * HEIGHT / 6) as f64, ((i + 1) * HEIGHT / 6) as f64);
        pic.fill(*color, |_, y| y >= top && y < bottom);
    }
    let generator = ImageColorGenerator::new(pic.width, pic.height, pic.rgb, None)?;
    Ok(ColorFunc::Image(Box::new(generator)))
}

fn builder(max_words: usize) -> AnyResult<wordcloud::WordCloudBuilder> {
    let config = Config {
        background_color: Some("white".into()),
        ..Config::default()
    };
    Ok(WordCloud::builder()
        .config(config)
        .mask(heart_mask()?)
        .color_func(rainbow()?)
        .max_words(max_words)
        .seed(SEED))
}

fn main() -> AnyResult {
    let wc = builder(400)?.build()?;
    let layout = wc.generate(&common::read("tests/golden/alice.txt")?)?;
    common::save(
        &render::to_image(&wc, &layout)?,
        &common::out("en", "image_colors.png"),
    )?;

    // Chinese: Dream of the Red Chamber (chapters 1-5).
    let wc = builder(200)?
        .rasterizer(common::font("ZCOOLKuaiLe-Regular.ttf")?)
        .build()?;
    let freqs = common::chinese_keywords("examples/data/zh/hongloumeng.txt", 200)?;
    let layout = wc.generate_from_frequencies(&freqs)?;
    common::save(
        &render::to_image(&wc, &layout)?,
        &common::out("zh", "image_colors.png"),
    )?;
    Ok(())
}

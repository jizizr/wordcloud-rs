//! Engine edge cases: errors, single words, repeat, recolor, trait defaults.

use wordcloud_rs::color::ColorFunc;
use wordcloud_rs::raster::{GlyphMask, Orientation, ReplayRasterizer, TextRasterizer};
use wordcloud_rs::{Config, Mask, WordCloud, WordCloudError, render, svg};

/// A trivial rasteriser: every character is a solid 1×size/2 block.
struct Blocks;
impl TextRasterizer for Blocks {
    fn bbox(&self, text: &str, size: u32, o: Orientation) -> wordcloud_rs::Result<(usize, usize)> {
        let (w, h) = (text.chars().count() * size as usize / 2, size as usize);
        Ok(if o == Orientation::Horizontal {
            (w, h)
        } else {
            (h, w)
        })
    }
    fn mask(&self, text: &str, size: u32, o: Orientation) -> wordcloud_rs::Result<GlyphMask> {
        let (w, h) = self.bbox(text, size, o)?;
        Ok(GlyphMask {
            width: w,
            height: h,
            data: vec![255; w * h],
        })
    }
}

fn wc(cfg: Config) -> WordCloud {
    WordCloud::builder()
        .config(cfg)
        .rasterizer(Box::new(Blocks))
        .color_func(ColorFunc::RandomHue)
        .build()
        .unwrap()
}

#[test]
fn errors_for_empty_input_and_full_mask() {
    let w = wc(Config {
        seed: Some(1),
        ..Config::default()
    });
    assert!(matches!(w.generate(""), Err(WordCloudError::NoWords)));
    let full = Mask {
        width: 50,
        height: 20,
        blocked: vec![true; 1000],
    };
    let w = WordCloud::builder()
        .config(Config {
            seed: Some(1),
            ..Config::default()
        })
        .rasterizer(Box::new(Blocks))
        .mask(full)
        .build()
        .unwrap();
    assert!(matches!(w.generate("a b c"), Err(WordCloudError::NoSpace)));
    assert!(
        WordCloudError::NoSpace
            .to_string()
            .contains("Couldn't find space")
    );
}

#[test]
fn single_word_starts_at_canvas_height_and_repeat_fills() {
    let w = wc(Config {
        seed: Some(2),
        height: 60,
        width: 300,
        ..Config::default()
    });
    let l = w.generate("solitary").unwrap();
    assert_eq!(l.words.len(), 1);
    assert!(l.words[0].font_size <= 60);
    let w = wc(Config {
        seed: Some(2),
        repeat: true,
        max_words: 30,
        relative_scaling: 0.0,
        ..Config::default()
    });
    let l = w.generate("solitary words").unwrap();
    assert!(l.words.len() > 2);
}

#[test]
fn explicit_max_font_size_vertical_only_and_unseeded() {
    let w = wc(Config {
        seed: Some(3),
        max_font_size: Some(30),
        prefer_horizontal: 0.0,
        ..Config::default()
    });
    let l = w.generate("alpha beta gamma delta").unwrap();
    assert!(l.words.iter().all(|p| p.font_size <= 30));
    assert!(
        l.words
            .iter()
            .any(|p| p.orientation == Orientation::Vertical)
    );
    let w = wc(Config {
        legacy_quirks: false,
        ..Config::default()
    });
    assert!(!w.generate("alpha beta").unwrap().words.is_empty());
}

#[test]
fn recolor_svg_and_render_with_custom_rasterizer() {
    let w = wc(Config {
        seed: Some(4),
        background_color: None,
        scale: 2.0,
        ..Config::default()
    });
    let l = w.generate("one two two three three three").unwrap();
    let r = w
        .recolor(&l, &ColorFunc::Constant("red".into()), Some(1))
        .unwrap();
    assert!(r.words.iter().all(|p| p.color == "red"));
    let r2 = w.recolor(&l, &ColorFunc::RandomHue, None).unwrap();
    assert_eq!(r2.words.len(), l.words.len());
    let doc = svg::to_svg(&w, &r).unwrap();
    assert!(doc.contains("sans-serif") && !doc.contains("<rect"));
    let img = render::to_image(&w, &r).unwrap();
    assert_eq!((img.width, img.height), (800, 400));
    let bad = w
        .recolor(&l, &ColorFunc::Constant("nope".into()), None)
        .unwrap();
    assert!(render::to_image(&w, &bad).is_err());
}

#[test]
fn replay_reports_missing_glyphs() {
    let rep = ReplayRasterizer::from_json(r#"{"bbox": {}, "mask": {}}"#).unwrap();
    assert!(rep.mask("x", 1, Orientation::Horizontal).is_err());
    let w = WordCloud::builder()
        .config(Config {
            seed: Some(1),
            ..Config::default()
        })
        .rasterizer(Box::new(rep))
        .build()
        .unwrap();
    let e = w.generate("hello world").unwrap_err();
    assert!(matches!(e, WordCloudError::MissingReplay(_)), "{e}");
}

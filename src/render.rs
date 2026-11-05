//! Final rendering: layout → RGB(A) pixels → PNG (port of `to_image`,
//! `_draw_contour`, `to_file`).

use crate::canvas::Canvas;
use crate::color::parse_color;
use crate::error::Result;
use crate::layout::{Layout, WordCloud};
use image::ImageEncoder;
use std::io::Write;

/// Renders a layout at `config.scale`, then draws the mask contour if enabled.
///
/// # Errors
/// Colour parsing or rasteriser errors.
///
/// ```
/// use wordcloud_rs::{render, WordCloud};
/// let wc = WordCloud::builder().size(200, 100).seed(3).build()?;
/// let layout = wc.generate("rust rust safe fast fast fast")?;
/// let img = render::to_image(&wc, &layout)?;
/// assert_eq!((img.width, img.height, img.channels), (200, 100, 3));
/// # Ok::<(), wordcloud_rs::WordCloudError>(())
/// ```
pub fn to_image(wc: &WordCloud, layout: &Layout) -> Result<Canvas> {
    let cfg = &wc.config;
    let (w, h) = wc.canvas_size();
    let (sw, sh) = (
        (w as f64 * cfg.scale) as usize,
        (h as f64 * cfg.scale) as usize,
    );
    let channels = if cfg.rgba { 4 } else { 3 };
    let bg = match &cfg.background_color {
        Some(c) => parse_color(c)?.0,
        None => [0; 4],
    };
    let mut img = Canvas::new(sw, sh, &bg[..channels]);
    for pw in &layout.words {
        let size = (f64::from(pw.font_size) * cfg.scale) as u32;
        let mask = wc.rasterizer().mask(&pw.word, size, pw.orientation)?;
        let ink = parse_color(&pw.color)?.0;
        let col = (pw.position.1 as f64 * cfg.scale) as i64;
        let row = (pw.position.0 as f64 * cfg.scale) as i64;
        img.draw_mask(&mask, col, row, &ink[..channels]);
    }
    if let Some(mask) = wc.mask().filter(|_| cfg.contour_width != 0.0) {
        let ink = parse_color(&cfg.contour_color)?.0;
        let edges = contour_pixels(
            &mask.blocked,
            mask.width,
            mask.height,
            sw,
            sh,
            cfg.contour_width,
        );
        for (i, on) in edges.into_iter().enumerate() {
            if on {
                img.data[i * channels..i * channels + 3].copy_from_slice(&ink[..3]);
            }
        }
    }
    Ok(img)
}

/// Pixels (at output size) covered by the blurred mask outline.
fn contour_pixels(
    blocked: &[bool],
    w: usize,
    h: usize,
    out_w: usize,
    out_h: usize,
    width: f64,
) -> Vec<bool> {
    let mut m: Vec<u8> = blocked.iter().map(|&b| if b { 255 } else { 0 }).collect();
    if (out_w, out_h) != (w, h) {
        let src = image::GrayImage::from_raw(w as u32, h as u32, m).unwrap_or_default();
        m = image::imageops::resize(
            &src,
            out_w as u32,
            out_h as u32,
            image::imageops::FilterType::CatmullRom,
        )
        .into_raw();
    }
    let (w, h) = (out_w, out_h);
    let mut edges = vec![0u8; w * h];
    for y in 1..h.saturating_sub(1) {
        for x in 1..w.saturating_sub(1) {
            let mut s = 8 * i32::from(m[y * w + x]);
            for (dy, dx) in [
                (0, 0),
                (0, 1),
                (0, 2),
                (1, 0),
                (1, 2),
                (2, 0),
                (2, 1),
                (2, 2),
            ] {
                s -= i32::from(m[(y + dy - 1) * w + x + dx - 1]);
            }
            edges[y * w + x] = s.clamp(0, 255) as u8;
        }
    }
    gaussian_blur(&mut edges, w, h, (width / 10.0) as f32);
    edges.into_iter().map(|v| v > 0).collect()
}

/// Pillow's `_gaussian_blur_radius` (3-pass extended box blur, f32 maths).
fn box_radius(radius: f32, passes: f32) -> f32 {
    let sigma2 = radius * radius / passes;
    let big_l = (12.0 * f64::from(sigma2) + 1.0).sqrt() as f32;
    let l = ((big_l - 1.0) / 2.0).floor();
    let a =
        (2.0 * l + 1.0) * (l * (l + 1.0) - 3.0 * sigma2) / (6.0 * (sigma2 - (l + 1.0) * (l + 1.0)));
    l + a
}

/// Pillow `ImagingLineBoxBlur8` for one line.
fn blur_line(input: &[u8], out: &mut [u8], float_radius: f32) {
    let len = input.len();
    let last = len - 1;
    let radius = float_radius as usize;
    let ww = (16_777_216.0 / (float_radius * 2.0 + 1.0)) as u32;
    let fw = (16_777_216 - (radius as u32 * 2 + 1) * ww) / 2;
    let edge_a = (radius + 1).min(len);
    let px = |i: usize| u32::from(input[i]);
    let mut acc = px(0) * (radius as u32 + 1);
    for x in 0..edge_a.saturating_sub(1) {
        acc += px(x);
    }
    acc += px(last) * (radius + 1 - edge_a) as u32;
    let mut step = |x: usize, sub: usize, add: usize, left: usize, right: usize| {
        acc = acc + px(add) - px(sub);
        let bulk = acc * ww + (px(left) + px(right)) * fw;
        out[x] = ((bulk + (1 << 23)) >> 24) as u8;
    };
    let far = |x: usize| (x + radius + 1).min(last);
    let near = |x: usize| (x + radius).min(last);
    for x in 0..len {
        let sub = if x < radius + 1 { 0 } else { x - radius - 1 };
        step(x, sub, near(x), sub, far(x));
    }
}

/// Pillow `GaussianBlur(radius)` on a grey buffer (in place).
fn gaussian_blur(buf: &mut [u8], w: usize, h: usize, radius: f32) {
    let r = box_radius(radius, 3.0);
    if r == 0.0 || w == 0 || h == 0 {
        return;
    }
    let mut line = Vec::new();
    for y in 0..h {
        for _ in 0..3 {
            line.clear();
            line.extend_from_slice(&buf[y * w..(y + 1) * w]);
            blur_line(&line, &mut buf[y * w..(y + 1) * w], r);
        }
    }
    let mut col_in = vec![0u8; h];
    let mut col_out = vec![0u8; h];
    for x in 0..w {
        for (y, v) in col_in.iter_mut().enumerate() {
            *v = buf[y * w + x];
        }
        for _ in 0..3 {
            blur_line(&col_in, &mut col_out, r);
            col_in.copy_from_slice(&col_out);
        }
        for (y, v) in col_in.iter().enumerate() {
            buf[y * w + x] = *v;
        }
    }
}

/// Encodes a canvas as PNG.
///
/// # Errors
/// I/O or encoding errors.
pub fn write_png<W: Write>(img: &Canvas, out: W) -> Result<()> {
    let ct = if img.channels == 4 {
        image::ExtendedColorType::Rgba8
    } else {
        image::ExtendedColorType::Rgb8
    };
    image::codecs::png::PngEncoder::new(out).write_image(
        &img.data,
        img.width as u32,
        img.height as u32,
        ct,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blur_radius_matches_pillow_formula() {
        assert_eq!(box_radius(0.0, 3.0), 0.0);
        assert!((box_radius(1.0, 3.0) - 0.25).abs() < 1e-6);
    }

    #[test]
    fn blur_spreads_and_preserves_flat() {
        let mut flat = vec![100u8; 25];
        gaussian_blur(&mut flat, 5, 5, 2.0);
        assert!(flat.iter().all(|&v| v == 100));
        let mut dot = vec![0u8; 49];
        dot[24] = 255;
        gaussian_blur(&mut dot, 7, 7, 1.0);
        assert!(dot[23] > 0 && dot[24] < 255);
    }

    #[test]
    fn contour_finds_mask_edge() {
        let (w, h) = (20, 20);
        let blocked: Vec<bool> = (0..w * h).map(|i| (i % w) >= 10).collect();
        let c = contour_pixels(&blocked, w, h, w, h, 3.0);
        assert!(c[5 * w + 10] || c[5 * w + 9]);
        assert!(!c[5 * w + 2]);
        let c2 = contour_pixels(&blocked, w, h, 2 * w, 2 * h, 3.0);
        assert_eq!(c2.len(), 4 * w * h);
    }
}

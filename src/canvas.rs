//! Pixel canvases with Pillow-exact alpha blending.
//!
//! Pillow's `ImageDraw.text` pastes an 8-bit coverage mask with
//! `fill_mask_L`, which blends every channel with the integer macro
//! `BLEND(m, a, b) = DIV255(a * (255 - m) + b * m)` where
//! `DIV255(v) = ((v + 128) >> 8) + (v + 128)) >> 8`. Using the same integer
//! formula keeps rendered pixels byte-identical to the legacy output.

use crate::raster::GlyphMask;

/// Pillow's `BLEND` macro.
///
/// ```
/// use wordcloud::canvas::blend;
/// assert_eq!(blend(255, 10, 200), 200);
/// assert_eq!(blend(0, 10, 200), 10);
/// assert_eq!(blend(128, 0, 255), 128);
/// ```
#[inline]
pub fn blend(mask: u8, under: u8, ink: u8) -> u8 {
    let v = u32::from(under) * (255 - u32::from(mask)) + u32::from(ink) * u32::from(mask);
    let t = v + 128;
    (((t >> 8) + t) >> 8) as u8
}

/// A row-major image with `channels` interleaved bytes per pixel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Canvas {
    /// Width in pixels.
    pub width: usize,
    /// Height in pixels.
    pub height: usize,
    /// Bytes per pixel (1 = grey, 3 = RGB, 4 = RGBA).
    pub channels: usize,
    /// Pixel data, `height * width * channels` bytes.
    pub data: Vec<u8>,
}

impl Canvas {
    /// Creates a canvas filled with `fill` (one value per channel).
    ///
    /// ```
    /// use wordcloud::canvas::Canvas;
    /// let c = Canvas::new(3, 2, &[1, 2, 3]);
    /// assert_eq!(c.data.len(), 18);
    /// assert_eq!(&c.data[3..6], &[1, 2, 3]);
    /// ```
    pub fn new(width: usize, height: usize, fill: &[u8]) -> Self {
        let channels = fill.len();
        let mut data = Vec::with_capacity(width * height * channels);
        for _ in 0..width * height {
            data.extend_from_slice(fill);
        }
        Self {
            width,
            height,
            channels,
            data,
        }
    }

    /// Blends `mask` with colour `ink` so that the mask's top-left pixel lands
    /// on column `col`, row `row`. Pixels outside the canvas are clipped.
    pub fn draw_mask(&mut self, mask: &GlyphMask, col: i64, row: i64, ink: &[u8]) {
        debug_assert_eq!(ink.len(), self.channels);
        let ch = self.channels;
        for my in 0..mask.height {
            let y = row + my as i64;
            if y < 0 || y >= self.height as i64 {
                continue;
            }
            let src = &mask.data[my * mask.width..(my + 1) * mask.width];
            let base = y as usize * self.width;
            for (mx, &m) in src.iter().enumerate() {
                let x = col + mx as i64;
                if m == 0 || x < 0 || x >= self.width as i64 {
                    continue;
                }
                let px = (base + x as usize) * ch;
                for (c, &k) in ink.iter().enumerate() {
                    self.data[px + c] = blend(m, self.data[px + c], k);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blend_matches_reference_formula() {
        for m in [0u8, 1, 64, 127, 128, 200, 254, 255] {
            for a in [0u8, 17, 128, 255] {
                for b in [0u8, 99, 255] {
                    let exact =
                        (f64::from(a) * f64::from(255 - m) + f64::from(b) * f64::from(m)) / 255.0;
                    assert!((f64::from(blend(m, a, b)) - exact).abs() <= 1.0);
                }
            }
        }
    }

    #[test]
    fn draw_mask_clips() {
        let mut c = Canvas::new(3, 3, &[0]);
        let mask = GlyphMask {
            width: 2,
            height: 2,
            data: vec![255; 4],
        };
        c.draw_mask(&mask, 2, 2, &[255]);
        c.draw_mask(&mask, -1, -1, &[255]);
        assert_eq!(c.data, vec![255, 0, 0, 0, 0, 0, 0, 0, 255]);
    }
}

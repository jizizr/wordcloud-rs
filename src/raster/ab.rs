//! Pure safe-Rust rasteriser built on `ab_glyph`.
//!
//! Glyphs are laid out left-to-right with advances and kerning, the union of
//! their pixel bounds forms the ink box, and outlines are rasterised with
//! anti-aliasing into that box. Pillow sizes fonts with
//! `FT_Set_Pixel_Sizes(size)` (pixels per em); `ab_glyph`'s `PxScale` is in
//! pixels per (ascent − descent), so the scale is converted accordingly.
//!
//! Results are close to, but not bit-identical with, FreeType + HarfBuzz
//! (hinting and anti-aliasing differ), which is why exact A/B parity uses
//! [`super::ReplayRasterizer`].

use super::{GlyphMask, Orientation, SvgMetrics, TextRasterizer};
use crate::error::{Result, WordCloudError};
use ab_glyph::{Font, FontArc, GlyphId, PxScale, ScaleFont, point};
use std::cell::RefCell;
use std::collections::HashMap;

/// Font bundled with the legacy package (Droid Sans Mono, Apache-2.0).
pub const DEFAULT_FONT: &[u8] = include_bytes!("../../assets/DroidSansMono.ttf");

type Key = (String, u32);

/// `ab_glyph`-based [`TextRasterizer`] with an optional bounding-box cache.
pub struct AbGlyphRasterizer {
    font: FontArc,
    family: (String, String),
    cache: Option<RefCell<HashMap<Key, (usize, usize)>>>,
}

/// Horizontal ink box of a word plus its rasterised coverage.
struct Ink {
    x0: i32,
    y0: i32,
    mask: GlyphMask,
}

impl AbGlyphRasterizer {
    /// Loads a TrueType/OpenType font from bytes.
    ///
    /// # Errors
    /// [`WordCloudError::InvalidFont`] if the bytes are not a font.
    ///
    /// ```
    /// use wordcloud_rs::raster::{AbGlyphRasterizer, Orientation, TextRasterizer};
    /// let r = AbGlyphRasterizer::from_bytes(wordcloud_rs::raster::DEFAULT_FONT.to_vec(), true)?;
    /// let (w, h) = r.bbox("Alice", 40, Orientation::Horizontal)?;
    /// assert!(w > h && h > 20);
    /// assert_eq!(r.bbox("Alice", 40, Orientation::Vertical)?, (h, w));
    /// # Ok::<(), wordcloud_rs::WordCloudError>(())
    /// ```
    pub fn from_bytes(bytes: Vec<u8>, cache: bool) -> Result<Self> {
        let family = font_names(&bytes);
        let font =
            FontArc::try_from_vec(bytes).map_err(|e| WordCloudError::InvalidFont(e.to_string()))?;
        Ok(Self {
            font,
            family,
            cache: cache.then(|| RefCell::new(HashMap::new())),
        })
    }

    /// Loads the bundled default font.
    ///
    /// # Errors
    /// Never fails in practice (the bundled font is valid).
    pub fn default_font(cache: bool) -> Result<Self> {
        Self::from_bytes(DEFAULT_FONT.to_vec(), cache)
    }

    fn scale(&self, size: u32) -> PxScale {
        let upem = self.font.units_per_em().unwrap_or(1000.0);
        PxScale::from(size as f32 * self.font.height_unscaled() / upem)
    }

    /// Lays out and outlines the glyphs of `text`, returning them with the
    /// integer ink box `(x0, y0, x1, y1)` relative to (pen origin, ascender line).
    fn outline(&self, text: &str, size: u32) -> (Vec<ab_glyph::OutlinedGlyph>, [i32; 4]) {
        let scale = self.scale(size);
        let sf = self.font.as_scaled(scale);
        let ascent = sf.ascent();
        let mut caret = 0.0f32;
        let mut prev: Option<GlyphId> = None;
        let mut outlined = Vec::new();
        for ch in text.chars() {
            let id = sf.glyph_id(ch);
            if let Some(p) = prev {
                caret += sf.kern(p, id);
            }
            let glyph = id.with_scale_and_position(scale, point(caret, ascent));
            caret += sf.h_advance(id);
            prev = Some(id);
            if let Some(og) = self.font.outline_glyph(glyph) {
                outlined.push(og);
            }
        }
        if outlined.is_empty() {
            return (outlined, [0; 4]);
        }
        let mut b = [i32::MAX, i32::MAX, i32::MIN, i32::MIN];
        for og in &outlined {
            let r = og.px_bounds();
            b[0] = b[0].min(r.min.x.floor() as i32);
            b[1] = b[1].min(r.min.y.floor() as i32);
            b[2] = b[2].max(r.max.x.ceil() as i32);
            b[3] = b[3].max(r.max.y.ceil() as i32);
        }
        (outlined, b)
    }

    fn render(&self, text: &str, size: u32) -> Ink {
        let (outlined, [x0, y0, x1, y1]) = self.outline(text, size);
        let (w, h) = ((x1 - x0) as usize, (y1 - y0) as usize);
        let mut data = vec![0u8; w * h];
        for og in &outlined {
            let b = og.px_bounds();
            let (ox, oy) = (b.min.x.floor() as i32 - x0, b.min.y.floor() as i32 - y0);
            og.draw(|x, y, c| {
                let (px, py) = (ox + x as i32, oy + y as i32);
                if px >= 0 && py >= 0 && (px as usize) < w && (py as usize) < h {
                    let cell = &mut data[py as usize * w + px as usize];
                    *cell = cell.saturating_add((c.clamp(0.0, 1.0) * 255.0).round() as u8);
                }
            });
        }
        Ink {
            x0,
            y0,
            mask: GlyphMask {
                width: w,
                height: h,
                data,
            },
        }
    }

    /// Horizontal `(width, height)` of the ink box, memoised when caching is on.
    fn measure(&self, text: &str, size: u32) -> (usize, usize) {
        let compute = || {
            let (_, [x0, y0, x1, y1]) = self.outline(text, size);
            ((x1 - x0) as usize, (y1 - y0) as usize)
        };
        let Some(cache) = &self.cache else {
            return compute();
        };
        let key = (text.to_string(), size);
        if let Some(&v) = cache.borrow().get(&key) {
            return v;
        }
        let v = compute();
        cache.borrow_mut().insert(key, v);
        v
    }
}

impl TextRasterizer for AbGlyphRasterizer {
    fn bbox(&self, text: &str, size: u32, orientation: Orientation) -> Result<(usize, usize)> {
        let (w, h) = self.measure(text, size);
        Ok(match orientation {
            Orientation::Horizontal => (w, h),
            Orientation::Vertical => (h, w),
        })
    }

    fn mask(&self, text: &str, size: u32, orientation: Orientation) -> Result<GlyphMask> {
        let m = self.render(text, size).mask;
        Ok(match orientation {
            Orientation::Horizontal => m,
            Orientation::Vertical => m.rotate_ccw(),
        })
    }

    fn family_and_style(&self) -> (String, String) {
        self.family.clone()
    }

    fn svg_metrics(&self, text: &str, size: u32) -> Result<SvgMetrics> {
        let ink = self.render(text, size);
        let ascent = f64::from(self.font.as_scaled(self.scale(size)).ascent());
        Ok(SvgMetrics {
            size_x: ink.mask.width as f64,
            offset_x: f64::from(ink.x0),
            offset_y: f64::from(ink.y0),
            ascent,
        })
    }
}

/// Reads the family (name ID 1) and subfamily (name ID 2) from the `name` table.
fn font_names(bytes: &[u8]) -> (String, String) {
    let fallback = || ("sans-serif".to_string(), "Regular".to_string());
    let Ok(face) = ttf_parser::Face::parse(bytes, 0) else {
        return fallback();
    };
    let get = |id: u16| {
        face.names()
            .into_iter()
            .filter(|n| n.name_id == id)
            .find_map(|n| n.to_string())
    };
    match (get(1), get(2)) {
        (Some(f), Some(s)) => (f, s),
        (Some(f), None) => (f, "Regular".into()),
        _ => fallback(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_and_space_text_have_no_ink() {
        let r = AbGlyphRasterizer::default_font(false).unwrap();
        assert_eq!(r.bbox("", 30, Orientation::Horizontal).unwrap(), (0, 0));
        assert_eq!(r.bbox(" ", 30, Orientation::Horizontal).unwrap(), (0, 0));
    }

    #[test]
    fn cached_bbox_matches_mask() {
        let r = AbGlyphRasterizer::default_font(true).unwrap();
        let a = r.mask("Queen", 25, Orientation::Vertical).unwrap();
        assert_eq!(
            r.bbox("Queen", 25, Orientation::Vertical).unwrap(),
            (a.width, a.height)
        );
        assert_eq!(
            r.bbox("Queen", 25, Orientation::Vertical).unwrap(),
            (a.width, a.height)
        );
        assert!(a.data.contains(&255));
    }

    #[test]
    fn reads_family_name_and_rejects_garbage() {
        let r = AbGlyphRasterizer::default_font(false).unwrap();
        assert_eq!(r.family_and_style().0, "Droid Sans Mono");
        assert!(AbGlyphRasterizer::from_bytes(vec![1, 2, 3], false).is_err());
        assert_eq!(font_names(&[0, 1]).0, "sans-serif");
        let m = r.svg_metrics("Hi", 20).unwrap();
        assert!(m.size_x > 0.0 && m.ascent > 0.0);
    }
}

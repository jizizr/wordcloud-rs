//! Text measurement and rasterisation backends.
//!
//! The layout algorithm only needs two operations from a font engine:
//! the ink bounding box of a word at a pixel size and orientation, and the
//! 8-bit coverage mask of that ink box. The legacy code got both from Pillow
//! (`textbbox(..., anchor="lt")` and `TransposedFont.getmask`), i.e.
//! FreeType + HarfBuzz/raqm. Two backends implement [`TextRasterizer`]:
//!
//! * [`AbGlyphRasterizer`] — pure safe Rust (`ab_glyph`), used in production;
//! * [`ReplayRasterizer`] — replays masks recorded from Pillow, used to prove
//!   the *rest* of the pipeline is byte-for-byte identical to the legacy code.

mod ab;
mod replay;

pub use ab::{AbGlyphRasterizer, DEFAULT_FONT};
pub use replay::ReplayRasterizer;

use crate::error::Result;

/// Text direction of a placed word.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Orientation {
    /// Normal left-to-right text.
    Horizontal,
    /// Rotated 90° counter-clockwise (Pillow `Transpose.ROTATE_90`).
    Vertical,
}

impl Orientation {
    /// The other orientation.
    ///
    /// ```
    /// use wordcloud_rs::raster::Orientation;
    /// assert_eq!(Orientation::Horizontal.flipped(), Orientation::Vertical);
    /// ```
    pub fn flipped(self) -> Self {
        match self {
            Self::Horizontal => Self::Vertical,
            Self::Vertical => Self::Horizontal,
        }
    }
}

/// An 8-bit coverage mask, row-major, `width * height` bytes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GlyphMask {
    /// Width in pixels.
    pub width: usize,
    /// Height in pixels.
    pub height: usize,
    /// Coverage values 0..=255.
    pub data: Vec<u8>,
}

impl GlyphMask {
    /// Rotates the mask 90° counter-clockwise (Pillow `ROTATE_90`).
    ///
    /// ```
    /// use wordcloud_rs::raster::GlyphMask;
    /// let m = GlyphMask { width: 2, height: 1, data: vec![1, 2] };
    /// let r = m.rotate_ccw();
    /// assert_eq!((r.width, r.height, r.data), (1, 2, vec![2, 1]));
    /// ```
    pub fn rotate_ccw(&self) -> Self {
        let (w, h) = (self.width, self.height);
        let mut data = vec![0u8; w * h];
        for r in 0..w {
            for c in 0..h {
                data[r * h + c] = self.data[c * w + (w - 1 - r)];
            }
        }
        Self {
            width: h,
            height: w,
            data,
        }
    }
}

/// Font metrics needed for SVG export (Pillow `font.getsize` + `getmetrics`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SvgMetrics {
    /// Ink width.
    pub size_x: f64,
    /// Horizontal ink offset from the pen origin.
    pub offset_x: f64,
    /// Vertical ink offset from the ascender line.
    pub offset_y: f64,
    /// Font ascent in pixels.
    pub ascent: f64,
}

/// A font engine able to measure and rasterise words.
pub trait TextRasterizer {
    /// `(width, height)` of the ink box of `text` at pixel `size` and `orientation`.
    ///
    /// # Errors
    /// Backend specific (e.g. a missing replay recording).
    fn bbox(&self, text: &str, size: u32, orientation: Orientation) -> Result<(usize, usize)>;

    /// Coverage mask of the ink box, already rotated for `orientation`.
    ///
    /// # Errors
    /// Backend specific (e.g. a missing replay recording).
    fn mask(&self, text: &str, size: u32, orientation: Orientation) -> Result<GlyphMask>;

    /// Font family name and style (for SVG export).
    fn family_and_style(&self) -> (String, String) {
        ("sans-serif".into(), "Regular".into())
    }

    /// Metrics for SVG export.
    ///
    /// # Errors
    /// Backend specific.
    fn svg_metrics(&self, text: &str, size: u32) -> Result<SvgMetrics> {
        let (w, h) = self.bbox(text, size, Orientation::Horizontal)?;
        Ok(SvgMetrics {
            size_x: w as f64,
            offset_x: 0.0,
            offset_y: 0.0,
            ascent: h as f64,
        })
    }
}

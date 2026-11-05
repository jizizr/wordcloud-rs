//! Replays glyph measurements and masks recorded from Pillow.
//!
//! `tools/record_pillow.py` runs the legacy Python package with
//! `TransposedFont.getbbox`/`getmask` instrumented and writes every result to
//! JSON. Feeding those into the Rust engine isolates the font engine from the
//! rest of the pipeline: with identical glyphs, layout, colours and pixels
//! must match the legacy output exactly. A request the recording does not
//! contain means the Rust engine asked for a different word/size/orientation
//! than Python did — i.e. a divergence — and is reported as an error.
//!
//! Recording format: `{"bbox": {key: [w, h]}, "mask": {key: [w, h, hex]}}`
//! with `key = "{size}|{0 horizontal / 1 vertical}|{text}"`.

use super::{GlyphMask, Orientation, TextRasterizer};
use crate::error::{Result, WordCloudError};
use std::collections::HashMap;

#[derive(serde::Deserialize)]
struct Recording {
    bbox: HashMap<String, (usize, usize)>,
    mask: HashMap<String, (usize, usize, String)>,
}

/// [`TextRasterizer`] backed by a Pillow recording.
pub struct ReplayRasterizer {
    bbox: HashMap<String, (usize, usize)>,
    mask: HashMap<String, (usize, usize, String)>,
}

fn key(text: &str, size: u32, orientation: Orientation) -> String {
    let o = u8::from(orientation == Orientation::Vertical);
    format!("{size}|{o}|{text}")
}

fn decode_hex(hex: &str) -> Option<Vec<u8>> {
    if !hex.len().is_multiple_of(2) {
        return None;
    }
    (0..hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(hex.get(i..i + 2)?, 16).ok())
        .collect()
}

impl ReplayRasterizer {
    /// Parses a recording.
    ///
    /// # Errors
    /// [`WordCloudError::Json`] if the JSON is malformed.
    ///
    /// ```
    /// use wordcloud_rs::raster::{Orientation, ReplayRasterizer, TextRasterizer};
    /// let json = r#"{"bbox": {"10|0|hi": [3, 2]}, "mask": {"10|0|hi": [3, 2, "00ff00ff00ff"]}}"#;
    /// let r = ReplayRasterizer::from_json(json)?;
    /// assert_eq!(r.bbox("hi", 10, Orientation::Horizontal)?, (3, 2));
    /// assert_eq!(r.mask("hi", 10, Orientation::Horizontal)?.data, vec![0, 255, 0, 255, 0, 255]);
    /// assert!(r.bbox("hi", 11, Orientation::Horizontal).is_err());
    /// # Ok::<(), wordcloud_rs::WordCloudError>(())
    /// ```
    pub fn from_json(json: &str) -> Result<Self> {
        let rec: Recording = serde_json::from_str(json)?;
        Ok(Self {
            bbox: rec.bbox,
            mask: rec.mask,
        })
    }
}

impl TextRasterizer for ReplayRasterizer {
    fn bbox(&self, text: &str, size: u32, orientation: Orientation) -> Result<(usize, usize)> {
        let k = key(text, size, orientation);
        self.bbox
            .get(&k)
            .copied()
            .ok_or(WordCloudError::MissingReplay(k))
    }

    fn mask(&self, text: &str, size: u32, orientation: Orientation) -> Result<GlyphMask> {
        let k = key(text, size, orientation);
        let (width, height, hex) = self
            .mask
            .get(&k)
            .ok_or_else(|| WordCloudError::MissingReplay(k.clone()))?;
        let data = decode_hex(hex)
            .filter(|d| d.len() == width * height)
            .ok_or(WordCloudError::MissingReplay(k))?;
        Ok(GlyphMask {
            width: *width,
            height: *height,
            data,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bad_hex_is_rejected() {
        assert_eq!(decode_hex("0"), None);
        assert_eq!(decode_hex("zz"), None);
        let r = ReplayRasterizer::from_json(r#"{"bbox": {}, "mask": {"1|1|a": [1, 1, "zz"]}}"#)
            .unwrap();
        assert!(r.mask("a", 1, Orientation::Vertical).is_err());
        assert!(ReplayRasterizer::from_json("nope").is_err());
    }
}

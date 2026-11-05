//! Colour words by the mean colour of an image region (port of
//! `color_from_image.py::ImageColorGenerator`).
//!
//! Legacy quirk preserved for parity: the patch is sliced as
//! `image[row .. row + bbox_width, col .. col + bbox_height]`, i.e. the word's
//! width is applied to the row axis (see the audit, bug #5).

use crate::color::ColorRequest;
use crate::error::{Result, WordCloudError};
use crate::raster::TextRasterizer;

/// An RGB image used as a colour source.
#[derive(Debug, Clone)]
pub struct ImageColorGenerator {
    width: usize,
    height: usize,
    rgb: Vec<u8>,
    default_color: Option<[u8; 3]>,
}

impl ImageColorGenerator {
    /// Creates a generator from row-major RGB bytes.
    ///
    /// # Errors
    /// [`WordCloudError::InvalidConfig`] if `rgb.len() != width * height * 3`.
    ///
    /// ```
    /// use wordcloud_rs::image_color::ImageColorGenerator;
    /// assert!(ImageColorGenerator::new(2, 2, vec![0; 12], None).is_ok());
    /// assert!(ImageColorGenerator::new(2, 2, vec![0; 5], None).is_err());
    /// ```
    pub fn new(
        width: usize,
        height: usize,
        rgb: Vec<u8>,
        default_color: Option<[u8; 3]>,
    ) -> Result<Self> {
        if rgb.len() != width * height * 3 {
            return Err(WordCloudError::InvalidConfig(format!(
                "colour image buffer has {} bytes, expected {}",
                rgb.len(),
                width * height * 3
            )));
        }
        Ok(Self {
            width,
            height,
            rgb,
            default_color,
        })
    }

    /// Mean colour (truncated, `"rgb(r, g, b)"`) of the patch under the word.
    ///
    /// # Errors
    /// [`WordCloudError::ColorImageTooSmall`] if the patch is empty and no
    /// default colour was configured; rasteriser errors are propagated.
    pub fn color(&self, req: &ColorRequest<'_>, raster: &dyn TextRasterizer) -> Result<String> {
        let (bw, bh) = raster.bbox(req.word, req.font_size, req.orientation)?;
        let (x, y) = req.position;
        let rows = x.min(self.height)..(x + bw).min(self.height);
        let cols = y.min(self.width)..(y + bh).min(self.width);
        let n = rows.len() * cols.len();
        if n == 0 {
            let [r, g, b] = self
                .default_color
                .ok_or(WordCloudError::ColorImageTooSmall)?;
            return Ok(format!("rgb({r}, {g}, {b})"));
        }
        let mut sum = [0u64; 3];
        for r in rows {
            for c in cols.clone() {
                let p = (r * self.width + c) * 3;
                for (k, s) in sum.iter_mut().enumerate() {
                    *s += u64::from(self.rgb[p + k]);
                }
            }
        }
        let [r, g, b] = sum.map(|s| (s as f64 / n as f64) as u64);
        Ok(format!("rgb({r}, {g}, {b})"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raster::{AbGlyphRasterizer, Orientation};

    #[test]
    fn mean_and_fallbacks() {
        let r = AbGlyphRasterizer::default_font(false).unwrap();
        let px: Vec<u8> = (0..100 * 100)
            .flat_map(|i| [(i % 256) as u8, 10, 20])
            .collect();
        let g = ImageColorGenerator::new(100, 100, px.clone(), None).unwrap();
        let req = ColorRequest {
            word: "ab",
            font_size: 10,
            position: (5, 5),
            orientation: Orientation::Horizontal,
        };
        assert!(g.color(&req, &r).unwrap().ends_with(", 10, 20)"));
        let far = ColorRequest {
            position: (500, 500),
            ..req
        };
        assert!(matches!(
            g.color(&far, &r),
            Err(WordCloudError::ColorImageTooSmall)
        ));
        let g = ImageColorGenerator::new(100, 100, px, Some([1, 2, 3])).unwrap();
        assert_eq!(g.color(&far, &r).unwrap(), "rgb(1, 2, 3)");
    }
}

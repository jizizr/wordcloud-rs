//! Per-word colour functions (ports of `random_color_func`,
//! `colormap_color_func`, `get_single_color_func` and `ImageColorGenerator`).
//!
//! Like the legacy callables they return Pillow colour *strings*
//! (`"hsl(…)"`, `"rgb(…)"`) which are stored in the layout and parsed only at
//! render time, so both the layout dump and the RNG consumption match Python.

use super::{colormaps::COLORMAPS, hsv_to_rgb, parse_color, rgb_to_hsv};
use crate::error::{Result, WordCloudError};
use crate::image_color::ImageColorGenerator;
use crate::raster::{Orientation, TextRasterizer};
use crate::rng::PyRandom;

/// What a colour function knows about the word being coloured.
#[derive(Debug, Clone, Copy)]
pub struct ColorRequest<'a> {
    /// The word or phrase.
    pub word: &'a str,
    /// Font size in pixels.
    pub font_size: u32,
    /// `(row, col)` of the word's top-left corner.
    pub position: (usize, usize),
    /// Orientation of the word.
    pub orientation: Orientation,
}

/// Strategy used to colour each word.
pub enum ColorFunc {
    /// Random hue, 80% saturation, 50% lightness (`random_color_func`).
    RandomHue,
    /// Uniform sample from a matplotlib colormap LUT (`colormap_color_func`).
    Colormap(&'static [[f64; 3]; 256]),
    /// Fixed hue/saturation of a colour with random value (`get_single_color_func`).
    Single {
        /// Hue in `[0, 1)`.
        h: f64,
        /// Saturation in `[0, 1]`.
        s: f64,
    },
    /// Always the same colour string.
    Constant(String),
    /// Mean colour of an image region under the word (`ImageColorGenerator`).
    Image(Box<ImageColorGenerator>),
}

impl ColorFunc {
    /// Looks up a matplotlib colormap by name (case-sensitive, like matplotlib).
    ///
    /// # Errors
    /// [`WordCloudError::InvalidConfig`] for unknown names.
    ///
    /// ```
    /// use wordcloud::color::ColorFunc;
    /// assert!(ColorFunc::colormap("viridis").is_ok());
    /// assert!(ColorFunc::colormap("nope").is_err());
    /// ```
    pub fn colormap(name: &str) -> Result<Self> {
        COLORMAPS
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, lut)| Self::Colormap(lut))
            .ok_or_else(|| WordCloudError::InvalidConfig(format!("unknown colormap {name:?}")))
    }

    /// Equivalent of `get_single_color_func(color)`.
    ///
    /// # Errors
    /// [`WordCloudError::InvalidColor`] if `color` cannot be parsed.
    pub fn single(color: &str) -> Result<Self> {
        let [r, g, b, _] = parse_color(color)?.0;
        let (h, s, _) = rgb_to_hsv(
            f64::from(r) / 255.0,
            f64::from(g) / 255.0,
            f64::from(b) / 255.0,
        );
        Ok(Self::Single { h, s })
    }

    /// Produces the colour string for one word, consuming `rng` exactly as
    /// the legacy function would.
    ///
    /// # Errors
    /// Propagates rasteriser errors and [`WordCloudError::ColorImageTooSmall`].
    ///
    /// ```
    /// use wordcloud::{color::*, raster::*, rng::PyRandom};
    /// let r = AbGlyphRasterizer::default_font(false)?;
    /// let req = ColorRequest { word: "hi", font_size: 10, position: (0, 0), orientation: Orientation::Horizontal };
    /// // random.Random(0).randint(0, 255) == 197
    /// assert_eq!(ColorFunc::RandomHue.color(&req, &mut PyRandom::new(0), &r)?, "hsl(197, 80%, 50%)");
    /// # Ok::<(), wordcloud::WordCloudError>(())
    /// ```
    pub fn color(
        &self,
        req: &ColorRequest<'_>,
        rng: &mut PyRandom,
        raster: &dyn TextRasterizer,
    ) -> Result<String> {
        Ok(match self {
            Self::RandomHue => format!("hsl({}, 80%, 50%)", rng.randint(0, 255)),
            Self::Colormap(lut) => {
                let x = rng.uniform(0.0, 1.0);
                let idx = ((x * 256.0) as usize).min(255);
                let [r, g, b] = lut[idx].map(|c| (255.0 * c).max(0.0));
                format!("rgb({r:.0}, {g:.0}, {b:.0})")
            }
            Self::Single { h, s } => {
                let (r, g, b) = hsv_to_rgb(*h, *s, rng.uniform(0.2, 1.0));
                format!("rgb({:.0}, {:.0}, {:.0})", r * 255.0, g * 255.0, b * 255.0)
            }
            Self::Constant(c) => c.clone(),
            Self::Image(generator) => generator.color(req, raster)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::raster::AbGlyphRasterizer;

    #[test]
    fn colormap_and_single_and_constant() {
        let r = AbGlyphRasterizer::default_font(false).unwrap();
        let req = ColorRequest {
            word: "a",
            font_size: 5,
            position: (0, 0),
            orientation: Orientation::Horizontal,
        };
        let mut rng = PyRandom::new(3);
        let c = ColorFunc::colormap("viridis")
            .unwrap()
            .color(&req, &mut rng, &r)
            .unwrap();
        assert!(c.starts_with("rgb("));
        let c = ColorFunc::single("deepskyblue")
            .unwrap()
            .color(&req, &mut rng, &r)
            .unwrap();
        assert!(parse_color(&c).is_ok());
        let c = ColorFunc::Constant("white".into())
            .color(&req, &mut rng, &r)
            .unwrap();
        assert_eq!(c, "white");
        assert!(ColorFunc::single("bogus").is_err());
    }
}

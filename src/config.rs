//! Configuration, masks and the [`WordCloudBuilder`].
//!
//! Defaults follow the Python `WordCloud.__init__` signature.

use crate::color::ColorFunc;
use crate::error::{Result, WordCloudError};
use crate::layout::WordCloud;
use crate::raster::{AbGlyphRasterizer, TextRasterizer};
use crate::text::TextOptions;

/// All tunables of a word cloud.
#[derive(Debug, Clone)]
pub struct Config {
    /// Canvas width (ignored when a mask is set).
    pub width: usize,
    /// Canvas height (ignored when a mask is set).
    pub height: usize,
    /// Free space kept around each word, in pixels.
    pub margin: usize,
    /// Probability of trying a word horizontally first.
    pub prefer_horizontal: f64,
    /// Render scale relative to the layout canvas.
    pub scale: f64,
    /// Maximum number of words.
    pub max_words: usize,
    /// Smallest font size; layout stops when a word no longer fits at it.
    pub min_font_size: i64,
    /// Largest font size; `None` derives it from the top two words.
    pub max_font_size: Option<i64>,
    /// Font-size decrement when a word does not fit.
    pub font_step: i64,
    /// 0 = size by rank only, 1 = size proportional to frequency.
    pub relative_scaling: f64,
    /// Repeat words until `max_words` or `min_font_size` is reached.
    pub repeat: bool,
    /// RNG seed; `None` = non-deterministic.
    pub seed: Option<i64>,
    /// Background colour; `None` = transparent/black.
    pub background_color: Option<String>,
    /// Render RGBA instead of RGB.
    pub rgba: bool,
    /// Mask contour width (0 = none).
    pub contour_width: f64,
    /// Mask contour colour.
    pub contour_color: String,
    /// Reproduce legacy bugs #1/#2 (needed for seed-exact parity).
    pub legacy_quirks: bool,
    /// Tokenisation options.
    pub text: TextOptions,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            width: 400,
            height: 200,
            margin: 2,
            prefer_horizontal: 0.9,
            scale: 1.0,
            max_words: 200,
            min_font_size: 4,
            max_font_size: None,
            font_step: 1,
            relative_scaling: 0.5,
            repeat: false,
            seed: None,
            background_color: Some("black".into()),
            rgba: false,
            contour_width: 0.0,
            contour_color: "black".into(),
            legacy_quirks: true,
            text: TextOptions::default(),
        }
    }
}

/// A boolean "do not draw here" mask, row-major.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mask {
    /// Width in pixels.
    pub width: usize,
    /// Height in pixels.
    pub height: usize,
    /// `true` where drawing is forbidden.
    pub blocked: Vec<bool>,
}

impl Mask {
    /// Builds a mask from interleaved pixels: a pixel is blocked when it is
    /// pure white (`== 255` for 1 channel, first three channels `== 255` otherwise).
    ///
    /// # Errors
    /// [`WordCloudError::InvalidConfig`] on a size mismatch or zero channels.
    ///
    /// ```
    /// use wordcloud_rs::Mask;
    /// let m = Mask::from_pixels(2, 1, 3, &[255, 255, 255, 0, 255, 255])?;
    /// assert_eq!(m.blocked, vec![true, false]);
    /// # Ok::<(), wordcloud_rs::WordCloudError>(())
    /// ```
    pub fn from_pixels(width: usize, height: usize, channels: usize, data: &[u8]) -> Result<Self> {
        if channels == 0 || data.len() != width * height * channels {
            return Err(WordCloudError::InvalidConfig(
                "mask buffer size mismatch".into(),
            ));
        }
        let used = channels.min(3);
        let blocked = data
            .chunks_exact(channels)
            .map(|px| px[..used].iter().all(|&v| v == 255))
            .collect();
        Ok(Self {
            width,
            height,
            blocked,
        })
    }
}

/// Fluent builder for [`WordCloud`].
///
/// ```
/// use wordcloud_rs::WordCloud;
/// let wc = WordCloud::builder().size(300, 150).max_words(50).seed(1).build()?;
/// let layout = wc.generate("alpha beta beta gamma gamma gamma")?;
/// assert_eq!(layout.words[0].word, "gamma");
/// # Ok::<(), wordcloud_rs::WordCloudError>(())
/// ```
pub struct WordCloudBuilder {
    config: Config,
    mask: Option<Mask>,
    raster: Option<Box<dyn TextRasterizer>>,
    color_func: Option<ColorFunc>,
}

impl WordCloud {
    /// Starts a builder with legacy defaults.
    pub fn builder() -> WordCloudBuilder {
        WordCloudBuilder {
            config: Config::default(),
            mask: None,
            raster: None,
            color_func: None,
        }
    }
}

impl WordCloudBuilder {
    /// Replaces the whole configuration.
    pub fn config(mut self, config: Config) -> Self {
        self.config = config;
        self
    }

    /// Canvas size.
    pub fn size(mut self, width: usize, height: usize) -> Self {
        self.config.width = width;
        self.config.height = height;
        self
    }

    /// Maximum number of words.
    pub fn max_words(mut self, n: usize) -> Self {
        self.config.max_words = n;
        self
    }

    /// RNG seed.
    pub fn seed(mut self, seed: i64) -> Self {
        self.config.seed = Some(seed);
        self
    }

    /// Blocking mask (overrides the canvas size).
    pub fn mask(mut self, mask: Mask) -> Self {
        self.mask = Some(mask);
        self
    }

    /// Font engine (defaults to the bundled font via `ab_glyph`).
    pub fn rasterizer(mut self, raster: Box<dyn TextRasterizer>) -> Self {
        self.raster = Some(raster);
        self
    }

    /// Colour function (defaults to the `viridis` colormap, as in Python).
    pub fn color_func(mut self, f: ColorFunc) -> Self {
        self.color_func = Some(f);
        self
    }

    /// Validates and builds.
    ///
    /// # Errors
    /// [`WordCloudError::InvalidConfig`] for out-of-range values.
    pub fn build(self) -> Result<WordCloud> {
        let mut config = self.config;
        if !(0.0..=1.0).contains(&config.relative_scaling) {
            return Err(WordCloudError::InvalidConfig(format!(
                "relative_scaling needs to be between 0 and 1, got {}.",
                config.relative_scaling
            )));
        }
        if config.font_step <= 0 || config.scale <= 0.0 {
            return Err(WordCloudError::InvalidConfig(
                "font_step and scale must be positive".into(),
            ));
        }
        if let Some(m) = &self.mask {
            config.width = m.width;
            config.height = m.height;
        }
        let raster = match self.raster {
            Some(r) => r,
            None => Box::new(AbGlyphRasterizer::default_font(true)?),
        };
        let color_func = match self.color_func {
            Some(f) => f,
            None => ColorFunc::colormap("viridis")?,
        };
        Ok(WordCloud {
            config,
            mask: self.mask,
            raster,
            color_func,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation() {
        let bad = Config {
            relative_scaling: 1.5,
            ..Config::default()
        };
        assert!(WordCloud::builder().config(bad).build().is_err());
        let bad = Config {
            font_step: 0,
            ..Config::default()
        };
        assert!(WordCloud::builder().config(bad).build().is_err());
        assert!(Mask::from_pixels(2, 2, 1, &[0; 3]).is_err());
        let m = Mask::from_pixels(3, 1, 1, &[255, 0, 255]).unwrap();
        let wc = WordCloud::builder().mask(m).build().unwrap();
        assert_eq!(wc.canvas_size(), (3, 1));
    }
}

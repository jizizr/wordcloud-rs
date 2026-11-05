//! The word-placement engine (port of `WordCloud.generate_from_frequencies`).
//!
//! Words are placed from most to least frequent. For each word the font size
//! is derived from the previous one, an orientation is drawn, and the
//! occupancy map is asked for a random free window; on failure the word is
//! rotated once and then shrunk by `font_step` until it fits or drops below
//! `min_font_size`, which ends the layout.
//!
//! Every random draw happens in the same order as in the legacy code
//! (including the hidden first-two-words probe that picks the starting font
//! size), so a given seed yields the identical layout.

use crate::canvas::Canvas;
use crate::color::{ColorFunc, ColorRequest};
use crate::config::{Config, Mask};
use crate::error::{Result, WordCloudError};
use crate::occupancy::{OccupancyMap, Pixels};
use crate::raster::{Orientation, TextRasterizer};
use crate::rng::PyRandom;
use crate::text::process_text;

/// One placed word.
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct PlacedWord {
    /// Word or phrase.
    pub word: String,
    /// Frequency normalised so the most frequent word is 1.
    pub freq: f64,
    /// Font size in pixels (layout scale).
    pub font_size: u32,
    /// `(row, col)` of the ink box's top-left corner.
    pub position: (usize, usize),
    /// Orientation.
    pub orientation: Orientation,
    /// Pillow colour string.
    pub color: String,
}

/// The fitted word cloud (`layout_` and `words_` in the legacy API).
#[derive(Debug, Clone, PartialEq, Default, serde::Serialize)]
pub struct Layout {
    /// Placed words in placement order.
    pub words: Vec<PlacedWord>,
    /// Normalised input frequencies after truncation to `max_words`.
    pub frequencies: Vec<(String, f64)>,
}

/// A configured word-cloud generator.
pub struct WordCloud {
    /// Configuration (canvas, sizing, text options…).
    pub config: Config,
    pub(crate) mask: Option<Mask>,
    pub(crate) raster: Box<dyn TextRasterizer>,
    pub(crate) color_func: ColorFunc,
}

/// Python's `round()` (ties to even) followed by `int()`.
fn py_round(v: f64) -> i64 {
    v.round_ties_even() as i64
}

impl WordCloud {
    /// The font engine in use.
    pub fn rasterizer(&self) -> &dyn TextRasterizer {
        self.raster.as_ref()
    }

    /// Canvas `(width, height)` in layout pixels (the mask's size if any).
    pub fn canvas_size(&self) -> (usize, usize) {
        self.mask
            .as_ref()
            .map_or((self.config.width, self.config.height), |m| {
                (m.width, m.height)
            })
    }

    /// The blocking mask, if any.
    pub fn mask(&self) -> Option<&Mask> {
        self.mask.as_ref()
    }

    fn new_rng(&self) -> PyRandom {
        self.config
            .seed
            .map_or_else(PyRandom::from_entropy, PyRandom::new)
    }

    /// Tokenises `text` and lays out the resulting frequencies.
    ///
    /// # Errors
    /// [`WordCloudError::NoWords`] if no words survive, [`WordCloudError::NoSpace`]
    /// if nothing fits, or rasteriser/colour errors.
    pub fn generate(&self, text: &str) -> Result<Layout> {
        let counts = process_text(text, &self.config.text)?;
        let freqs: Vec<(String, f64)> = counts.into_iter().map(|(w, c)| (w, c as f64)).collect();
        self.generate_from_frequencies(&freqs)
    }

    /// Lays out explicit `(word, frequency)` pairs.
    ///
    /// # Errors
    /// As [`WordCloud::generate`].
    pub fn generate_from_frequencies(&self, freqs: &[(String, f64)]) -> Result<Layout> {
        self.fit(freqs, None, &mut self.new_rng())
    }

    fn starting_font_size(
        &self,
        freqs: &[(String, f64)],
        max_font_size: Option<i64>,
        rng: &mut PyRandom,
    ) -> Result<i64> {
        if let Some(m) = max_font_size.or(self.config.max_font_size) {
            return Ok(m);
        }
        if freqs.len() == 1 {
            return Ok(self.canvas_size().1 as i64);
        }
        // Legacy probe: lay out the top two words at max size = canvas height
        // (consuming RNG draws) and use the harmonic mean of their sizes.
        let probe = self.fit(&freqs[..2], Some(self.canvas_size().1 as i64), rng)?;
        match probe.words.as_slice() {
            [a, b, ..] => {
                let (a, b) = (f64::from(a.font_size), f64::from(b.font_size));
                Ok((2.0 * a * b / (a + b)) as i64)
            }
            [a] => Ok(i64::from(a.font_size)),
            [] => Err(WordCloudError::NoSpace),
        }
    }

    fn fit(
        &self,
        input: &[(String, f64)],
        max_font_size: Option<i64>,
        rng: &mut PyRandom,
    ) -> Result<Layout> {
        let cfg = &self.config;
        let mut sorted = input.to_vec();
        sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        sorted.truncate(cfg.max_words);
        let Some(max_freq) = sorted.first().map(|(_, f)| *f) else {
            return Err(WordCloudError::NoWords);
        };
        let mut freqs: Vec<(String, f64)> =
            sorted.into_iter().map(|(w, f)| (w, f / max_freq)).collect();

        let (width, height) = self.canvas_size();
        let blocked = self.mask.as_ref().map(|m| m.blocked.as_slice());
        let mut occupancy = OccupancyMap::new(height, width, blocked);
        let mut canvas = Canvas::new(width, height, &[0]);
        let mut font_size = self.starting_font_size(&freqs, max_font_size, rng)?;
        let mut layout = Layout {
            words: Vec::new(),
            frequencies: freqs.clone(),
        };

        if cfg.repeat && freqs.len() < cfg.max_words {
            let times_extend = (cfg.max_words as f64 / freqs.len() as f64).ceil() as usize - 1;
            let original = freqs.clone();
            let downweight = original.last().map_or(1.0, |f| f.1);
            for i in 0..times_extend {
                let factor = downweight.powf((i + 1) as f64);
                freqs.extend(original.iter().map(|(w, f)| (w.clone(), f * factor)));
            }
        }

        let mut last_freq = 1.0;
        for (word, freq) in &freqs {
            if *freq == 0.0 {
                continue;
            }
            let rs = cfg.relative_scaling;
            if rs != 0.0 {
                font_size = py_round((rs * (freq / last_freq) + (1.0 - rs)) * font_size as f64);
            }
            let mut orientation = if rng.random() < cfg.prefer_horizontal {
                Orientation::Horizontal
            } else {
                Orientation::Vertical
            };
            let mut tried_other = false;
            let mut found = None;
            while font_size >= cfg.min_font_size {
                let (bw, bh) = self.raster.bbox(word, font_size as u32, orientation)?;
                found = occupancy.sample_position(
                    bh + cfg.margin,
                    bw + cfg.margin,
                    rng,
                    cfg.legacy_quirks,
                );
                if found.is_some() {
                    break;
                }
                if !tried_other && cfg.prefer_horizontal < 1.0 {
                    orientation = if cfg.legacy_quirks {
                        Orientation::Vertical
                    } else {
                        orientation.flipped()
                    };
                    tried_other = true;
                } else {
                    font_size -= cfg.font_step;
                    orientation = Orientation::Horizontal;
                }
            }
            let Some((rx, ry)) = found.filter(|_| font_size >= cfg.min_font_size) else {
                break;
            };
            let (x, y) = (rx + cfg.margin / 2, ry + cfg.margin / 2);
            let size = font_size as u32;
            let glyph = self.raster.mask(word, size, orientation)?;
            canvas.draw_mask(&glyph, y as i64, x as i64, &[255]);
            let req = ColorRequest {
                word,
                font_size: size,
                position: (x, y),
                orientation,
            };
            let color = self.color_func.color(&req, rng, self.raster.as_ref())?;
            layout.words.push(PlacedWord {
                word: word.clone(),
                freq: *freq,
                font_size: size,
                position: (x, y),
                orientation,
                color,
            });
            let pixels = match blocked {
                None => Pixels::Canvas(&canvas.data),
                Some(mask) => Pixels::Masked {
                    canvas: &canvas.data,
                    mask,
                    wrap: cfg.legacy_quirks,
                },
            };
            occupancy.update(x, y, pixels);
            last_freq = *freq;
        }
        Ok(layout)
    }

    /// Re-colours an existing layout (legacy `recolor`), returning a new layout.
    ///
    /// # Errors
    /// Colour-function errors.
    pub fn recolor(
        &self,
        layout: &Layout,
        color_func: &ColorFunc,
        seed: Option<i64>,
    ) -> Result<Layout> {
        let mut rng = seed.map_or_else(PyRandom::from_entropy, PyRandom::new);
        let mut out = layout.clone();
        for w in &mut out.words {
            let req = ColorRequest {
                word: &w.word,
                font_size: w.font_size,
                position: w.position,
                orientation: w.orientation,
            };
            w.color = color_func.color(&req, &mut rng, self.raster.as_ref())?;
        }
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn py_round_is_bankers() {
        assert_eq!(py_round(2.5), 2);
        assert_eq!(py_round(3.5), 4);
        assert_eq!(py_round(-0.5), 0);
    }
}

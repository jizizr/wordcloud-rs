//! Integral-image occupancy map and the free-position sampling kernel.
//!
//! Port of `IntegralOccupancyMap` (NumPy) and `query_integral_image.pyx`
//! (Cython, `boundscheck=False`). The integral image is stored as one flat,
//! row-major `Vec<u32>` (structure-of-arrays, cache friendly) and indexed
//! through bounds-checked slices instead of raw memoryviews.
//!
//! ## Arithmetic
//! The legacy integral is `uint32` and silently wraps for very large canvases.
//! Inclusion–exclusion is still exact modulo 2³², so every operation here uses
//! `wrapping_*` arithmetic: an area is "free" iff it is `0 mod 2³²`, exactly
//! like the Cython `if not area` test.
//!
//! ## Window semantics
//! For a candidate `(i, j)` the kernel tests the rectangle of rows
//! `i+1 ..= i+size_x` and columns `j+1 ..= j+size_y` (the integral has no
//! zero padding), and candidates are `0 <= i < height - size_x`,
//! `0 <= j < width - size_y`.

use crate::rng::PyRandom;

mod naive;

/// Source of per-pixel occupancy values for an integral update.
#[derive(Debug, Clone, Copy)]
pub enum Pixels<'a> {
    /// Grey canvas values.
    Canvas(&'a [u8]),
    /// Canvas plus 1 per masked pixel; `wrap` reproduces the legacy `uint8`
    /// overflow (bug #3).
    Masked {
        /// Grey canvas values.
        canvas: &'a [u8],
        /// Blocked pixels.
        mask: &'a [bool],
        /// Wrap `u8` addition like NumPy does.
        wrap: bool,
    },
    /// Explicit values.
    Values(&'a [u32]),
}

impl Pixels<'_> {
    /// Value of flat pixel `i`.
    pub fn at(&self, i: usize) -> u32 {
        match *self {
            Pixels::Canvas(c) => u32::from(c[i]),
            Pixels::Masked {
                canvas,
                mask,
                wrap: true,
            } => u32::from(canvas[i].wrapping_add(u8::from(mask[i]))),
            Pixels::Masked {
                canvas,
                mask,
                wrap: false,
            } => u32::from(canvas[i]) + u32::from(mask[i]),
            Pixels::Values(v) => v[i],
        }
    }
}

/// One row of the integral recurrence: running row sum plus the row above.
/// Monomorphised per pixel source so the inner loop has no branches.
#[inline]
fn accumulate(
    out: &mut [u32],
    up: Option<&[u32]>,
    mut row_sum: u32,
    px: impl Iterator<Item = u32>,
) {
    match up {
        Some(up) => {
            for ((cell, &u), p) in out.iter_mut().zip(up).zip(px) {
                row_sum = row_sum.wrapping_add(p);
                *cell = u.wrapping_add(row_sum);
            }
        }
        None => {
            for (cell, p) in out.iter_mut().zip(px) {
                row_sum = row_sum.wrapping_add(p);
                *cell = row_sum;
            }
        }
    }
}

/// Summed-area table of the occupied pixels of the canvas.
#[derive(Debug, Clone)]
pub struct OccupancyMap {
    height: usize,
    width: usize,
    integral: Vec<u32>,
    /// Hits per candidate row from the last counting pass (reused buffer).
    row_hits: Vec<u32>,
}

impl OccupancyMap {
    /// Creates the map. `mask` (row-major, `true` = blocked) seeds the table
    /// with `255` per blocked pixel, as the legacy `cumsum(255 * mask)` does.
    ///
    /// ```
    /// use wordcloud_rs::occupancy::OccupancyMap;
    /// let m = OccupancyMap::new(4, 5, None);
    /// assert_eq!((m.height(), m.width()), (4, 5));
    /// ```
    pub fn new(height: usize, width: usize, mask: Option<&[bool]>) -> Self {
        let mut map = Self {
            height,
            width,
            integral: vec![0; height * width],
            row_hits: Vec::new(),
        };
        if let Some(mask) = mask {
            let values: Vec<u32> = mask.iter().map(|&m| if m { 255 } else { 0 }).collect();
            map.recompute_from(0, 0, Pixels::Values(&values));
        }
        map
    }

    /// Canvas height (rows).
    pub fn height(&self) -> usize {
        self.height
    }

    /// Canvas width (columns).
    pub fn width(&self) -> usize {
        self.width
    }

    /// Raw summed-area value at `(row, col)`.
    pub fn integral_at(&self, row: usize, col: usize) -> u32 {
        self.integral[row * self.width + col]
    }

    /// Picks a uniformly random free `size_x × size_y` window.
    ///
    /// Draws `goal = randint(0, hits)` exactly like the legacy kernel. With
    /// `legacy_quirks` the legacy off-by-one is kept: `goal == 0` yields `None`
    /// even when space exists. Without it, `goal` is drawn from `0..hits`.
    ///
    /// ```
    /// use wordcloud_rs::{occupancy::OccupancyMap, rng::PyRandom};
    /// let m = OccupancyMap::new(10, 10, None);
    /// let pos = m.clone().sample_position(3, 3, &mut PyRandom::new(1), true);
    /// assert!(matches!(pos, Some((i, j)) if i < 7 && j < 7));
    /// ```
    pub fn sample_position(
        &mut self,
        size_x: usize,
        size_y: usize,
        rng: &mut PyRandom,
        legacy_quirks: bool,
    ) -> Option<(usize, usize)> {
        let rows = self.height.checked_sub(size_x)?;
        let cols = self.width.checked_sub(size_y)?;
        if cfg!(feature = "naive") {
            return self.sample_naive(rows, cols, size_x, size_y, rng, legacy_quirks);
        }
        let mut row_hits = std::mem::take(&mut self.row_hits);
        row_hits.clear();
        row_hits.extend((0..rows).map(|i| self.count_row(i, size_x, size_y, cols)));
        let hits: u64 = row_hits.iter().map(|&h| u64::from(h)).sum();
        let result = if hits == 0 {
            None
        } else {
            let goal = if legacy_quirks {
                rng.randint(0, hits as i64) as u64
            } else {
                rng.randbelow(hits) + 1
            };
            self.locate(&row_hits, goal, size_x, size_y, cols)
        };
        self.row_hits = row_hits;
        result
    }

    /// Number of free windows whose top-left candidate lies in row `i`.
    #[inline]
    fn count_row(&self, i: usize, size_x: usize, size_y: usize, cols: usize) -> u32 {
        let w = self.width;
        let top = &self.integral[i * w..(i + 1) * w];
        let bottom = &self.integral[(i + size_x) * w..(i + size_x + 1) * w];
        let (top_l, top_r) = (&top[..cols], &top[size_y..size_y + cols]);
        let (bot_l, bot_r) = (&bottom[..cols], &bottom[size_y..size_y + cols]);
        let mut n = 0u32;
        assert!(
            top_l.len() == cols
                && top_r.len() == cols
                && bot_l.len() == cols
                && bot_r.len() == cols
        );
        for k in 0..cols {
            // SAFETY: all four slices have length `cols` (asserted above) and k < cols.
            #[allow(unsafe_code)]
            let area = unsafe {
                top_l
                    .get_unchecked(k)
                    .wrapping_add(*bot_r.get_unchecked(k))
                    .wrapping_sub(bot_l.get_unchecked(k).wrapping_add(*top_r.get_unchecked(k)))
            };
            n += u32::from(area == 0);
        }
        n
    }

    /// Finds the `goal`-th (1-based) free window using per-row hit counts to
    /// skip whole rows instead of re-scanning them.
    fn locate(
        &self,
        row_hits: &[u32],
        goal: u64,
        size_x: usize,
        size_y: usize,
        cols: usize,
    ) -> Option<(usize, usize)> {
        let mut seen = 0u64;
        for (i, &h) in row_hits.iter().enumerate() {
            if seen + u64::from(h) < goal {
                seen += u64::from(h);
                continue;
            }
            let w = self.width;
            for j in 0..cols {
                let a = self.integral[i * w + j];
                let b = self.integral[(i + size_x) * w + j + size_y];
                let c = self.integral[(i + size_x) * w + j];
                let d = self.integral[i * w + j + size_y];
                if a.wrapping_add(b).wrapping_sub(c.wrapping_add(d)) == 0 {
                    seen += 1;
                    if seen == goal {
                        return Some((i, j));
                    }
                }
            }
            return None;
        }
        None
    }

    /// Recomputes the table for rows `>= pos_x` and columns `>= pos_y` after new
    /// ink was drawn there, reading occupancy values from `pixels`.
    pub fn update(&mut self, pos_x: usize, pos_y: usize, pixels: Pixels<'_>) {
        if cfg!(feature = "naive") {
            self.update_naive(pos_x, pos_y, |i| pixels.at(i));
        } else {
            self.recompute_from(pos_x, pos_y, pixels);
        }
    }

    /// In-place recurrence `I[i][j] = p + I[i-1][j] + I[i][j-1] - I[i-1][j-1]`
    /// over the bottom-right region; equivalent (mod 2³²) to the legacy
    /// "cumsum + paste border" formulation. Works row by row on slices so the
    /// inner loop is free of bounds checks.
    fn recompute_from(&mut self, pos_x: usize, pos_y: usize, pixels: Pixels<'_>) {
        let w = self.width;
        for i in pos_x..self.height {
            let (before, rest) = self.integral.split_at_mut(i * w);
            let row = &mut rest[..w];
            let above: Option<&[u32]> = if i > 0 {
                Some(&before[(i - 1) * w..])
            } else {
                None
            };
            let row_sum = if pos_y > 0 {
                row[pos_y - 1].wrapping_sub(above.map_or(0, |a| a[pos_y - 1]))
            } else {
                0
            };
            let (start, len) = (i * w + pos_y, w - pos_y);
            let up = above.map(|a| &a[pos_y..w]);
            let out = &mut row[pos_y..];
            match pixels {
                Pixels::Canvas(c) => accumulate(
                    out,
                    up,
                    row_sum,
                    c[start..start + len].iter().map(|&v| u32::from(v)),
                ),
                Pixels::Values(v) => {
                    accumulate(out, up, row_sum, v[start..start + len].iter().copied())
                }
                Pixels::Masked { canvas, mask, wrap } => {
                    let it = canvas[start..start + len]
                        .iter()
                        .zip(&mask[start..start + len]);
                    if wrap {
                        accumulate(
                            out,
                            up,
                            row_sum,
                            it.map(|(&c, &m)| u32::from(c.wrapping_add(u8::from(m)))),
                        );
                    } else {
                        accumulate(
                            out,
                            up,
                            row_sum,
                            it.map(|(&c, &m)| u32::from(c) + u32::from(m)),
                        );
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn brute_integral(h: usize, w: usize, px: &[u32]) -> Vec<u32> {
        let mut out = vec![0u32; h * w];
        for i in 0..h {
            for j in 0..w {
                let mut s = 0u32;
                for a in 0..=i {
                    for b in 0..=j {
                        s = s.wrapping_add(px[a * w + b]);
                    }
                }
                out[i * w + j] = s;
            }
        }
        out
    }

    #[test]
    fn mask_initialisation_matches_bruteforce() {
        let (h, w) = (5, 6);
        let mask: Vec<bool> = (0..h * w).map(|k| k % 7 == 0).collect();
        let m = OccupancyMap::new(h, w, Some(&mask));
        let px: Vec<u32> = mask.iter().map(|&b| if b { 255 } else { 0 }).collect();
        assert_eq!(m.integral, brute_integral(h, w, &px));
    }

    #[test]
    fn update_matches_bruteforce() {
        let (h, w) = (6, 7);
        let mut m = OccupancyMap::new(h, w, None);
        let mut px = vec![0u32; h * w];
        for (k, v) in px.iter_mut().enumerate() {
            if k % 5 == 0 && k / w >= 2 && k % w >= 3 {
                *v = 255;
            }
        }
        m.update(2, 3, Pixels::Values(&px));
        assert_eq!(m.integral, brute_integral(h, w, &px));
    }

    #[test]
    fn full_canvas_has_no_room() {
        let mask = vec![true; 25];
        let mut m = OccupancyMap::new(5, 5, Some(&mask));
        assert_eq!(m.sample_position(2, 2, &mut PyRandom::new(0), true), None);
        assert_eq!(m.sample_position(9, 2, &mut PyRandom::new(0), true), None);
    }

    #[test]
    fn fixed_mode_always_finds_space() {
        let mut m = OccupancyMap::new(10, 10, None);
        for s in 0..200 {
            assert!(
                m.sample_position(3, 3, &mut PyRandom::new(s), false)
                    .is_some()
            );
        }
    }
}

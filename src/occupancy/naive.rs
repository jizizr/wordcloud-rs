//! Literal transliterations of the legacy kernels.
//!
//! Compiled into every build but only *used* with the `naive` cargo feature,
//! which exists to benchmark the "initial safe Rust" stage of the migration.
//! The unit test below proves both implementations are interchangeable.

use super::OccupancyMap;
use crate::rng::PyRandom;

impl OccupancyMap {
    /// Literal transliteration of the Cython kernel: two full scans, no row skipping.
    pub(super) fn sample_naive(
        &self,
        rows: usize,
        cols: usize,
        size_x: usize,
        size_y: usize,
        rng: &mut PyRandom,
        legacy_quirks: bool,
    ) -> Option<(usize, usize)> {
        let w = self.width;
        let free = |i: usize, j: usize| {
            let a =
                self.integral[i * w + j].wrapping_add(self.integral[(i + size_x) * w + j + size_y]);
            a.wrapping_sub(
                self.integral[(i + size_x) * w + j].wrapping_add(self.integral[i * w + j + size_y]),
            ) == 0
        };
        let hits = (0..rows)
            .flat_map(|i| (0..cols).map(move |j| (i, j)))
            .filter(|&(i, j)| free(i, j))
            .count() as u64;
        if hits == 0 {
            return None;
        }
        let goal = if legacy_quirks {
            rng.randint(0, hits as i64) as u64
        } else {
            rng.randbelow(hits) + 1
        };
        (0..rows)
            .flat_map(|i| (0..cols).map(move |j| (i, j)))
            .filter(|&(i, j)| free(i, j))
            .nth(goal.checked_sub(1)? as usize)
    }

    /// NumPy-style update: allocate the region, cumsum along both axes, add
    /// the border rows/columns, paste back.
    pub(super) fn update_naive<F: Fn(usize) -> u32>(
        &mut self,
        pos_x: usize,
        pos_y: usize,
        pixel: F,
    ) {
        let (w, rh, rw) = (self.width, self.height - pos_x, self.width - pos_y);
        let mut part: Vec<u32> = (0..rh * rw)
            .map(|k| pixel((pos_x + k / rw) * w + pos_y + k % rw))
            .collect();
        for r in 0..rh {
            for c in 1..rw {
                part[r * rw + c] = part[r * rw + c].wrapping_add(part[r * rw + c - 1]);
            }
        }
        for r in 1..rh {
            for c in 0..rw {
                part[r * rw + c] = part[r * rw + c].wrapping_add(part[(r - 1) * rw + c]);
            }
        }
        for r in 0..rh {
            for c in 0..rw {
                let mut v = part[r * rw + c];
                if pos_x > 0 {
                    v = v.wrapping_add(self.integral[(pos_x - 1) * w + pos_y + c]);
                    if pos_y > 0 {
                        v = v.wrapping_sub(self.integral[(pos_x - 1) * w + pos_y - 1]);
                    }
                }
                if pos_y > 0 {
                    v = v.wrapping_add(self.integral[(pos_x + r) * w + pos_y - 1]);
                }
                self.integral[(pos_x + r) * w + pos_y + c] = v;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{OccupancyMap, Pixels};
    use crate::rng::PyRandom;

    #[test]
    fn naive_kernels_equal_optimised_kernels() {
        let (h, w) = (23, 31);
        let mut r = PyRandom::new(11);
        let blocked: Vec<bool> = (0..h * w).map(|_| r.randbelow(10) < 3).collect();
        let fast = OccupancyMap::new(h, w, Some(&blocked));
        for (sx, sy) in [(1, 1), (2, 3), (4, 2), (30, 1)] {
            for quirks in [true, false] {
                for seed in 0..20 {
                    let (rows, cols) = (h.saturating_sub(sx), w.saturating_sub(sy));
                    let a = fast
                        .clone()
                        .sample_position(sx, sy, &mut PyRandom::new(seed), quirks);
                    let b = fast.sample_naive(rows, cols, sx, sy, &mut PyRandom::new(seed), quirks);
                    assert_eq!(a, b, "size {sx}x{sy} seed {seed}");
                }
            }
        }
        let canvas: Vec<u8> = (0..h * w)
            .map(|k| if k % 7 == 0 { 255 } else { 0 })
            .collect();
        for (px, py) in [(0, 0), (0, 5), (4, 0), (9, 13)] {
            let pixels = Pixels::Masked {
                canvas: &canvas,
                mask: &blocked,
                wrap: true,
            };
            let mut a = fast.clone();
            let mut b = fast.clone();
            a.update(px, py, pixels);
            b.update_naive(px, py, |i| pixels.at(i));
            assert_eq!(a.integral, b.integral);
        }
        let p = Pixels::Masked {
            canvas: &canvas,
            mask: &blocked,
            wrap: false,
        };
        assert_eq!(p.at(0), 255 + u32::from(blocked[0]));
        assert_eq!(Pixels::Canvas(&canvas).at(7), 255);
        let mut c = fast.clone();
        c.update(3, 3, p);
        c.update(0, 0, Pixels::Canvas(&canvas));
        assert_eq!(
            c.integral_at(h - 1, w - 1),
            canvas.iter().map(|&v| u32::from(v)).sum::<u32>()
        );
    }
}

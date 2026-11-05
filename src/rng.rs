//! CPython-compatible pseudo-random number generator.
//!
//! The legacy implementation draws every random decision (orientation, position,
//! colour) from Python's [`random.Random`](https://docs.python.org/3/library/random.html),
//! a 32-bit Mersenne Twister (MT19937) with CPython-specific seeding and
//! range-reduction. Bit-exact reproduction of that stream is what makes
//! seed-for-seed A/B parity with the Python package possible, so this module
//! re-implements `Modules/_randommodule.c` and `Lib/random.py` exactly:
//!
//! * `seed(n)` → `init_by_array` over the 32-bit little-endian limbs of `|n|`;
//! * `random()` → 53-bit float from two draws: `(a >> 5) * 2^26 + (b >> 6)) / 2^53`;
//! * `randint(a, b)` → `a + _randbelow(b - a + 1)` via rejection sampling on
//!   `getrandbits(bit_length(n))`;
//! * `uniform(a, b)` → `a + (b - a) * random()`.
//!
//! ```
//! use wordcloud::rng::PyRandom;
//! let mut rng = PyRandom::new(42);
//! // Same value as `random.Random(42).random()` in CPython.
//! assert_eq!(rng.random(), 0.6394267984578837);
//! ```

const N: usize = 624;
const M: usize = 397;
const MATRIX_A: u32 = 0x9908_b0df;
const UPPER_MASK: u32 = 0x8000_0000;
const LOWER_MASK: u32 = 0x7fff_ffff;

/// A Mersenne Twister generator producing exactly the same stream as CPython's
/// `random.Random` for the same integer seed.
#[derive(Clone, Debug)]
pub struct PyRandom {
    mt: Box<[u32; N]>,
    index: usize,
}

impl PyRandom {
    /// Creates a generator equivalent to `random.Random(seed)`.
    ///
    /// Negative seeds use their absolute value, as CPython does.
    ///
    /// ```
    /// use wordcloud::rng::PyRandom;
    /// assert_eq!(PyRandom::new(-7).random(), PyRandom::new(7).random());
    /// ```
    pub fn new(seed: i64) -> Self {
        let n = seed.unsigned_abs();
        let key: Vec<u32> = if n >> 32 == 0 {
            vec![n as u32]
        } else {
            vec![n as u32, (n >> 32) as u32]
        };
        let mut rng = Self {
            mt: Box::new([0; N]),
            index: N,
        };
        rng.init_by_array(&key);
        rng
    }

    /// Creates a generator seeded from the system clock, mirroring the
    /// non-deterministic `random.Random()` used when no seed is supplied.
    pub fn from_entropy() -> Self {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let mixed = (nanos as u64) ^ ((nanos >> 64) as u64) ^ u64::from(std::process::id());
        Self::new((mixed >> 1) as i64)
    }

    fn init_genrand(&mut self, s: u32) {
        self.mt[0] = s;
        for i in 1..N {
            let prev = self.mt[i - 1];
            self.mt[i] = 1_812_433_253u32
                .wrapping_mul(prev ^ (prev >> 30))
                .wrapping_add(i as u32);
        }
        self.index = N;
    }

    fn init_by_array(&mut self, key: &[u32]) {
        self.init_genrand(19_650_218);
        let (mut i, mut j) = (1usize, 0usize);
        for _ in 0..N.max(key.len()) {
            let prev = self.mt[i - 1];
            self.mt[i] = (self.mt[i] ^ (prev ^ (prev >> 30)).wrapping_mul(1_664_525))
                .wrapping_add(key[j])
                .wrapping_add(j as u32);
            i += 1;
            j += 1;
            if i >= N {
                self.mt[0] = self.mt[N - 1];
                i = 1;
            }
            if j >= key.len() {
                j = 0;
            }
        }
        for _ in 0..N - 1 {
            let prev = self.mt[i - 1];
            self.mt[i] = (self.mt[i] ^ (prev ^ (prev >> 30)).wrapping_mul(1_566_083_941))
                .wrapping_sub(i as u32);
            i += 1;
            if i >= N {
                self.mt[0] = self.mt[N - 1];
                i = 1;
            }
        }
        self.mt[0] = 0x8000_0000;
    }

    fn twist(&mut self) {
        let mag = |y: u32| if y & 1 == 0 { 0 } else { MATRIX_A };
        for kk in 0..N {
            let y = (self.mt[kk] & UPPER_MASK) | (self.mt[(kk + 1) % N] & LOWER_MASK);
            self.mt[kk] = self.mt[(kk + M) % N] ^ (y >> 1) ^ mag(y);
        }
        self.index = 0;
    }

    /// Returns the next raw 32-bit output (`genrand_uint32`).
    pub fn next_u32(&mut self) -> u32 {
        if self.index >= N {
            self.twist();
        }
        let mut y = self.mt[self.index];
        self.index += 1;
        y ^= y >> 11;
        y ^= (y << 7) & 0x9d2c_5680;
        y ^= (y << 15) & 0xefc6_0000;
        y ^ (y >> 18)
    }

    /// Equivalent of `random.random()`: a float in `[0, 1)` with 53 random bits.
    pub fn random(&mut self) -> f64 {
        let a = self.next_u32() >> 5;
        let b = self.next_u32() >> 6;
        (f64::from(a) * 67_108_864.0 + f64::from(b)) * (1.0 / 9_007_199_254_740_992.0)
    }

    /// Equivalent of `random.getrandbits(k)` for `1 <= k <= 64`.
    ///
    /// # Panics
    /// Panics if `k` is 0 or greater than 64.
    pub fn getrandbits(&mut self, k: u32) -> u64 {
        assert!(
            (1..=64).contains(&k),
            "getrandbits supports 1..=64 bits, got {k}"
        );
        if k <= 32 {
            return u64::from(self.next_u32() >> (32 - k));
        }
        let low = u64::from(self.next_u32());
        let high = u64::from(self.next_u32() >> (64 - k));
        low | (high << 32)
    }

    /// Equivalent of `random._randbelow(n)`: uniform integer in `[0, n)`.
    ///
    /// # Panics
    /// Panics if `n == 0` (CPython raises `ValueError`).
    pub fn randbelow(&mut self, n: u64) -> u64 {
        assert!(n > 0, "randbelow requires n > 0");
        let k = 64 - n.leading_zeros();
        loop {
            let r = self.getrandbits(k);
            if r < n {
                return r;
            }
        }
    }

    /// Equivalent of `random.randint(a, b)`: uniform integer in `[a, b]` inclusive.
    ///
    /// ```
    /// use wordcloud::rng::PyRandom;
    /// let mut rng = PyRandom::new(0);
    /// let v = rng.randint(0, 255);
    /// assert!((0..=255).contains(&v));
    /// ```
    pub fn randint(&mut self, a: i64, b: i64) -> i64 {
        assert!(b >= a, "empty range for randint({a}, {b})");
        a + self.randbelow((b - a) as u64 + 1) as i64
    }

    /// Equivalent of `random.uniform(a, b)`.
    pub fn uniform(&mut self, a: f64, b: f64) -> f64 {
        a + (b - a) * self.random()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_cpython_reference_values() {
        // Reference values printed by CPython 3.13.
        assert_eq!(PyRandom::new(42).random(), 0.6394267984578837);
        assert_eq!(PyRandom::new(0).random(), 0.8444218515250481);
        let mut r = PyRandom::new(1);
        let v: Vec<i64> = (0..5).map(|_| r.randint(0, 255)).collect();
        assert_eq!(v, vec![68, 32, 130, 60, 253]);
    }

    #[test]
    fn large_seed_uses_two_limbs() {
        let a = PyRandom::new(1 << 40).random();
        let b = PyRandom::new(1).random();
        assert_ne!(a, b);
    }

    #[test]
    fn getrandbits_wide() {
        let mut r = PyRandom::new(3);
        assert!(r.getrandbits(40) < (1 << 40));
        assert!(r.getrandbits(64) > 0);
    }

    #[test]
    #[should_panic]
    fn randbelow_zero_panics() {
        PyRandom::new(0).randbelow(0);
    }

    #[test]
    fn entropy_seeded_generator_works() {
        let x = PyRandom::from_entropy().random();
        assert!((0.0..1.0).contains(&x));
    }
}

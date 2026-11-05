//! Property-based tests (proptest).

use proptest::prelude::*;
use wordcloud::occupancy::{OccupancyMap, Pixels};
use wordcloud::rng::PyRandom;
use wordcloud::text::{TextOptions, process_text};
use wordcloud::{Mask, WordCloud};

proptest! {
    #[test]
    fn randint_in_range(seed in any::<i64>(), a in -1000i64..1000, span in 0i64..100_000) {
        let mut r = PyRandom::new(seed);
        let v = r.randint(a, a + span);
        prop_assert!(v >= a && v <= a + span);
        let x = r.random();
        prop_assert!((0.0..1.0).contains(&x));
        let u = r.uniform(0.2, 1.0);
        prop_assert!((0.2..=1.0).contains(&u));
    }

    /// Every sampled window is really empty, for random masks and sizes.
    #[test]
    fn sampled_windows_are_free(
        h in 3usize..40, w in 3usize..40, density in 0u32..100, seed in any::<i64>(),
        sx in 1usize..10, sy in 1usize..10, quirks in any::<bool>(),
    ) {
        let mut r = PyRandom::new(seed);
        let blocked: Vec<bool> = (0..h * w).map(|_| (r.randbelow(100) as u32) < density).collect();
        let mut m = OccupancyMap::new(h, w, Some(&blocked));
        if let Some((i, j)) = m.sample_position(sx, sy, &mut r, quirks) {
            prop_assert!(i + sx < h && j + sy < w);
            for a in i + 1..=i + sx {
                for b in j + 1..=j + sy {
                    prop_assert!(!blocked[a * w + b]);
                }
            }
        } else if !quirks {
            // No result in fixed mode means there is really no free window.
            for i in 0..h.saturating_sub(sx) {
                for j in 0..w.saturating_sub(sy) {
                    let free = (i + 1..=i + sx).all(|a| (j + 1..=j + sy).all(|b| !blocked[a * w + b]));
                    prop_assert!(!free);
                }
            }
        }
    }

    /// Incremental integral updates equal a from-scratch build.
    #[test]
    fn incremental_update_equals_rebuild(h in 1usize..30, w in 1usize..30, px in 0usize..30, py in 0usize..30, seed in any::<i64>()) {
        let (px, py) = (px % h, py % w);
        let mut r = PyRandom::new(seed);
        let vals: Vec<u32> = (0..h * w).map(|k| if k / w >= px && k % w >= py { r.randbelow(256) as u32 } else { 0 }).collect();
        let mut m = OccupancyMap::new(h, w, None);
        m.update(px, py, Pixels::Values(&vals));
        let mut fresh = OccupancyMap::new(h, w, None);
        fresh.update(0, 0, Pixels::Values(&vals));
        for i in 0..h { for j in 0..w { prop_assert_eq!(m.integral_at(i, j), fresh.integral_at(i, j)); } }
    }

    #[test]
    fn tokenizer_invariants(text in "[a-zA-Z' .,0-9]{0,300}") {
        let opts = TextOptions::default();
        let counts = process_text(&text, &opts).unwrap();
        for (w, c) in &counts {
            prop_assert!(*c > 0);
            prop_assert!(!opts.stopwords.contains(&w.to_lowercase()));
            prop_assert!(!w.chars().all(|c| c.is_ascii_digit()));
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(12))]
    /// Layout is deterministic per seed, words never exceed the canvas, and
    /// masked pixels are never the anchor of a placed word.
    #[test]
    fn layouts_are_deterministic_and_in_bounds(seed in any::<i64>(), max_words in 1usize..60) {
        let blocked: Vec<bool> = (0..200 * 120).map(|k| (k % 200) < 20).collect();
        let build = || WordCloud::builder().size(200, 120).max_words(max_words).seed(seed)
            .mask(Mask { width: 200, height: 120, blocked: blocked.clone() }).build().unwrap();
        let text = "lorem ipsum dolor sit amet consectetur adipiscing elit sed do eiusmod tempor ipsum ipsum dolor";
        let a = build().generate(text).unwrap();
        prop_assert_eq!(&a, &build().generate(text).unwrap());
        for w in &a.words {
            prop_assert!(w.position.0 < 120 && w.position.1 < 200);
            prop_assert!(w.position.1 >= 20);
        }
    }
}

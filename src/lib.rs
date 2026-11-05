//! # wordcloud-rs
//!
//! A memory-safe Rust port of the Python [`wordcloud`](https://github.com/amueller/word_cloud)
//! package: tokenise text, count words and collocations, and pack them onto a
//! canvas with an integral-image occupancy search.
//!
//! All code is safe Rust except one audited, benchmark-justified `unsafe` block
//! (bounds-check elision in the occupancy kernel, guarded by an `assert!`).
//!
//! The engine is seed-for-seed compatible with the Python implementation: it
//! reproduces CPython's Mersenne Twister ([`rng::PyRandom`]), Python dict
//! ordering, Pillow's alpha blending and the legacy layout quirks
//! (configurable through [`Config::legacy_quirks`]).
//!
//! ```
//! use wordcloud_rs::{render, WordCloud};
//! let wc = WordCloud::builder().size(300, 150).seed(7).build()?;
//! let layout = wc.generate("safe fast safe rust rust rust")?;
//! assert_eq!(layout.words[0].word, "rust");
//! let png = {
//!     let mut buf = Vec::new();
//!     render::write_png(&render::to_image(&wc, &layout)?, &mut buf)?;
//!     buf
//! };
//! assert_eq!(&png[1..4], b"PNG");
//! # Ok::<(), wordcloud_rs::WordCloudError>(())
//! ```

// `unsafe` is denied crate-wide; the single audited exception is the bounds-check-free
// window-counting kernel in `occupancy.rs` (see its SAFETY comment).
#![deny(unsafe_code)]
#![warn(missing_docs)]

pub mod canvas;
pub mod cli;
pub mod color;
mod config;
mod error;
pub mod image_color;
mod layout;
pub mod occupancy;
pub mod raster;
pub mod render;
pub mod rng;
pub mod svg;
pub mod text;

pub use config::{Config, Mask, WordCloudBuilder};
pub use error::{Result, WordCloudError};
pub use layout::{Layout, PlacedWord, WordCloud};

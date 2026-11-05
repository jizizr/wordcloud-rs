//! Error type shared by the whole crate.

use thiserror::Error;

/// Everything that can go wrong while building, laying out or rendering a word cloud.
///
/// The legacy Python code signalled these conditions with a mix of
/// `ValueError`, `IndexError` and `NotImplementedError`; here they are one typed enum.
#[derive(Debug, Error)]
pub enum WordCloudError {
    /// No words survived tokenisation / the frequency map was empty.
    #[error("We need at least 1 word to plot a word cloud, got 0.")]
    NoWords,
    /// Not even the first word could be placed on the canvas.
    #[error(
        "Couldn't find space to draw. Either the Canvas size is too small or too much of the image is masked out."
    )]
    NoSpace,
    /// A configuration value was out of range.
    #[error("invalid configuration: {0}")]
    InvalidConfig(String),
    /// A colour specification could not be parsed.
    #[error("unknown color specifier: {0:?}")]
    InvalidColor(String),
    /// A regular expression supplied for tokenisation was invalid.
    #[error("invalid regular expression: {0}")]
    InvalidRegex(#[from] regex::Error),
    /// The font file could not be parsed.
    #[error("invalid font: {0}")]
    InvalidFont(String),
    /// The replay rasteriser was asked for a glyph it has no recording of.
    #[error("replay rasterizer has no recording for {0:?}")]
    MissingReplay(String),
    /// Colour image smaller than the canvas and no default colour supplied.
    #[error("ImageColorGenerator is smaller than the canvas")]
    ColorImageTooSmall,
    /// Image encoding/decoding failure.
    #[error("image error: {0}")]
    Image(#[from] image::ImageError),
    /// I/O failure.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    /// Malformed replay recording.
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Convenience alias used across the crate.
pub type Result<T> = std::result::Result<T, WordCloudError>;

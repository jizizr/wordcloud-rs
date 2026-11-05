//! Text → word-frequency pipeline (port of `WordCloud.process_text`).
//!
//! Steps, in legacy order:
//! 1. tokenise with the regex `\w[\w']*` (or `\w[\w']+` when `min_word_length > 1`,
//!    or a user regex); Python's `\w` is approximated by `[\p{L}\p{N}_]`;
//! 2. strip a trailing `'s`;
//! 3. drop pure-digit tokens unless `include_numbers`;
//! 4. drop tokens shorter than `min_word_length` (in code points);
//! 5. remove stopwords and count, with or without collocations.

mod counts;

pub use counts::{Counts, ProcessedTokens, process_tokens, score, unigrams_and_bigrams};

use crate::error::Result;
use regex::Regex;
use std::collections::HashSet;
use std::sync::LazyLock;

/// The default English stopword list shipped with the legacy package.
pub const DEFAULT_STOPWORDS: &str = include_str!("../../assets/stopwords");

/// Parses a newline-separated stopword list (each line stripped, as Python's `str.strip`).
pub fn parse_stopwords(list: &str) -> HashSet<String> {
    list.lines().map(|l| l.trim().to_string()).collect()
}

/// Options that control tokenisation and counting.
#[derive(Debug, Clone)]
pub struct TextOptions {
    /// Words removed before counting (compared case-insensitively).
    pub stopwords: HashSet<String>,
    /// Custom tokenisation regex; `None` uses the default word pattern.
    pub regexp: Option<String>,
    /// Detect and keep two-word collocations.
    pub collocations: bool,
    /// Merge `word`/`words` pairs.
    pub normalize_plurals: bool,
    /// Keep tokens consisting only of digits.
    pub include_numbers: bool,
    /// Minimum token length in characters (0 = no limit).
    pub min_word_length: usize,
    /// Dunning score a bigram must exceed to count as a collocation.
    pub collocation_threshold: f64,
}

impl Default for TextOptions {
    fn default() -> Self {
        Self {
            stopwords: parse_stopwords(DEFAULT_STOPWORDS),
            regexp: None,
            collocations: true,
            normalize_plurals: true,
            include_numbers: false,
            min_word_length: 0,
            collocation_threshold: 30.0,
        }
    }
}

static WORD_STAR: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[\p{L}\p{N}_][\p{L}\p{N}_']*").expect("static regex"));
static WORD_PLUS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[\p{L}\p{N}_][\p{L}\p{N}_']+").expect("static regex"));
static DIGITS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[\p{Nd}¹²³⁰-⁹₀-₉]+$").expect("static regex"));

/// Approximation of Python's `str.isdigit()` (decimal digits plus super/subscripts).
fn is_py_digit(word: &str) -> bool {
    DIGITS.is_match(word)
}

/// Splits `text` into tokens and returns the ordered word → count map.
///
/// # Errors
/// Returns [`crate::WordCloudError::InvalidRegex`] if `opts.regexp` is invalid.
///
/// ```
/// use wordcloud_rs::text::{process_text, TextOptions};
/// let counts = process_text("The cat's hat. The cats sat on 42 hats!", &TextOptions::default())?;
/// assert_eq!(counts.get("cat"), Some(&2));
/// assert_eq!(counts.get("hat"), Some(&2));
/// assert!(counts.get("42").is_none());
/// # Ok::<(), wordcloud_rs::WordCloudError>(())
/// ```
pub fn process_text(text: &str, opts: &TextOptions) -> Result<Counts> {
    let custom;
    let re: &Regex = match &opts.regexp {
        Some(p) => {
            custom = Regex::new(p)?;
            &custom
        }
        None if opts.min_word_length <= 1 => &WORD_STAR,
        None => &WORD_PLUS,
    };
    let mut words: Vec<String> = re
        .find_iter(text)
        .map(|m| {
            let w = m.as_str();
            let tail: String = w
                .chars()
                .rev()
                .take(2)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            if tail.to_lowercase() == "'s" {
                counts_drop(w)
            } else {
                w.to_string()
            }
        })
        .collect();
    if !opts.include_numbers {
        words.retain(|w| !is_py_digit(w));
    }
    if opts.min_word_length > 0 {
        words.retain(|w| w.chars().count() >= opts.min_word_length);
    }
    let stopwords: HashSet<String> = opts.stopwords.iter().map(|s| s.to_lowercase()).collect();
    if opts.collocations {
        Ok(unigrams_and_bigrams(
            &words,
            &stopwords,
            opts.normalize_plurals,
            opts.collocation_threshold,
        ))
    } else {
        words.retain(|w| !stopwords.contains(&w.to_lowercase()));
        Ok(process_tokens(&words, opts.normalize_plurals).counts)
    }
}

fn counts_drop(w: &str) -> String {
    counts::drop_last_chars(w, 2).to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digits_detection() {
        assert!(is_py_digit("123"));
        assert!(is_py_digit("²"));
        assert!(!is_py_digit("12a"));
    }

    #[test]
    fn min_length_switches_pattern() {
        let opts = TextOptions {
            min_word_length: 3,
            ..TextOptions::default()
        };
        let c = process_text("ox cow horse", &opts).unwrap();
        assert!(c.get("ox").is_none());
        assert_eq!(c.get("horse"), Some(&1));
    }

    #[test]
    fn numbers_kept_when_requested() {
        let opts = TextOptions {
            include_numbers: true,
            collocations: false,
            ..TextOptions::default()
        };
        let c = process_text("2024 2024 year", &opts).unwrap();
        assert_eq!(c.get("2024"), Some(&2));
    }

    #[test]
    fn custom_regex_and_errors() {
        let opts = TextOptions {
            regexp: Some(r"\w+".into()),
            ..TextOptions::default()
        };
        assert!(process_text("a-b", &opts).is_ok());
        let bad = TextOptions {
            regexp: Some("(".into()),
            ..TextOptions::default()
        };
        assert!(process_text("x", &bad).is_err());
    }
}

//! Case folding, plural merging and collocation (bigram) detection.
//!
//! Port of `wordcloud/tokenization.py`. Python dictionaries preserve insertion
//! order and the later `sorted(..., reverse=True)` is stable, so the *order* in
//! which words are first seen decides how frequency ties are broken in the
//! final layout. Every map here is therefore an [`IndexMap`] and every
//! iteration follows the legacy order exactly.

use indexmap::{IndexMap, IndexSet};
use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hasher};

/// Ordered word → count map (Python `dict` semantics).
pub type Counts = IndexMap<String, i64>;

/// FxHash (rustc's hasher): much faster than SipHash for the short keys counted
/// here, and HashDoS resistance is irrelevant for a local word count.
#[derive(Default, Clone, Copy)]
pub(crate) struct FxHasher(u64);

impl Hasher for FxHasher {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        let mut chunks = bytes.chunks_exact(8);
        for c in &mut chunks {
            self.add(u64::from_le_bytes(c.try_into().expect("8 bytes")));
        }
        let rest = chunks.remainder();
        if !rest.is_empty() {
            let mut buf = [0u8; 8];
            buf[..rest.len()].copy_from_slice(rest);
            self.add(u64::from_le_bytes(buf) ^ (rest.len() as u64) << 56);
        }
    }
    #[inline]
    fn write_u8(&mut self, i: u8) {
        self.add(u64::from(i));
    }
    #[inline]
    fn write_u32(&mut self, i: u32) {
        self.add(u64::from(i));
    }
    #[inline]
    fn write_u64(&mut self, i: u64) {
        self.add(i);
    }
    #[inline]
    fn write_usize(&mut self, i: usize) {
        self.add(i as u64);
    }
    #[inline]
    fn finish(&self) -> u64 {
        self.0
    }
}

impl FxHasher {
    #[inline]
    fn add(&mut self, word: u64) {
        self.0 = (self.0.rotate_left(5) ^ word).wrapping_mul(0x51_7c_c1_b7_27_22_0a_95);
    }
}

pub(crate) type FxBuild = BuildHasherDefault<FxHasher>;
pub(crate) type FxIndexSet<T> = IndexSet<T, FxBuild>;
pub(crate) type FxIndexMap<K, V> = IndexMap<K, V, FxBuild>;

/// Drops the last `n` characters (Python `s[:-n]` on code points).
pub(crate) fn drop_last_chars(s: &str, n: usize) -> &str {
    let mut end = s.len();
    for _ in 0..n {
        match s[..end].char_indices().next_back() {
            Some((i, _)) => end = i,
            None => return "",
        }
    }
    &s[..end]
}

/// Dunning log-likelihood term `log(max(x,1e-10))*k + log(max(1-x,1e-10))*(n-k)`.
fn l(k: f64, n: f64, x: f64) -> f64 {
    x.max(1e-10).ln() * k + (1.0 - x).max(1e-10).ln() * (n - k)
}

/// Dunning likelihood-ratio collocation score for a bigram.
///
/// Returns 0 when one of the words makes up the whole document.
///
/// ```
/// use wordcloud::text::score;
/// assert_eq!(score(1, 10, 3, 10), 0.0);
/// assert!(score(20, 25, 22, 1000) > 30.0);
/// ```
pub fn score(count_bigram: i64, count1: i64, count2: i64, n_words: i64) -> f64 {
    if n_words <= count1 || n_words <= count2 {
        return 0.0;
    }
    let (n, c12, c1, c2) = (
        n_words as f64,
        count_bigram as f64,
        count1 as f64,
        count2 as f64,
    );
    let p = c2 / n;
    let p1 = c12 / c1;
    let p2 = (c2 - c12) / (n - c1);
    let s = l(c12, c1, p) + l(c2 - c12, n - c1, p) - l(c12, c1, p1) - l(c2 - c12, n - c1, p2);
    -2.0 * s
}

/// Result of [`process_tokens`].
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ProcessedTokens {
    /// Count per word, keyed by the word's most common capitalisation.
    pub counts: Counts,
    /// Lower-case word (including merged plurals) → standard capitalisation.
    pub standard_forms: HashMap<String, String>,
}

/// Normalises case and (optionally) merges simple plurals.
///
/// Each word is represented by its most common capitalisation (first seen wins
/// ties). If `normalize_plurals` is set, a lower-case key ending in `s` but not
/// `ss` is merged into its singular when the singular also occurs.
///
/// ```
/// use wordcloud::text::process_tokens;
/// let words = ["Cat", "cats", "cat", "dog"].map(String::from);
/// let p = process_tokens(&words, true);
/// assert_eq!(p.counts["cat"], 3);
/// assert_eq!(p.standard_forms["cats"], "cat");
/// ```
pub fn process_tokens(words: &[String], normalize_plurals: bool) -> ProcessedTokens {
    let mut distinct: FxIndexMap<&str, i64> = FxIndexMap::default();
    for w in words {
        *distinct.entry(w.as_str()).or_insert(0) += 1;
    }
    process_weighted(distinct.into_iter(), normalize_plurals)
}

/// [`process_tokens`] on distinct words with their counts, in first-seen order.
///
/// The legacy algorithm only depends on the order in which each word is first
/// seen and on how often it occurs, so counting duplicates up front gives the
/// same result while lower-casing each distinct word only once.
pub(crate) fn process_weighted<S: AsRef<str>>(
    words: impl Iterator<Item = (S, i64)>,
    normalize_plurals: bool,
) -> ProcessedTokens {
    let mut d: FxIndexMap<String, IndexMap<String, i64>> = FxIndexMap::default();
    for (word, count) in words {
        let word = word.as_ref();
        *d.entry(word.to_lowercase())
            .or_default()
            .entry(word.to_string())
            .or_insert(0) += count;
    }
    let mut merged_plurals: Vec<(String, String)> = Vec::new();
    if normalize_plurals {
        let keys: Vec<String> = d.keys().cloned().collect();
        for key in keys {
            if !(key.ends_with('s') && !key.ends_with("ss")) {
                continue;
            }
            let singular_key = drop_last_chars(&key, 1).to_string();
            if !d.contains_key(&singular_key) {
                continue;
            }
            let plural_cases = d.shift_remove(&key).unwrap_or_default();
            let singular_cases = d.get_mut(&singular_key).expect("checked above");
            for (word, count) in plural_cases {
                *singular_cases
                    .entry(drop_last_chars(&word, 1).to_string())
                    .or_insert(0) += count;
            }
            merged_plurals.push((key, singular_key));
        }
    }
    let mut out = ProcessedTokens::default();
    for (word_lower, cases) in &d {
        let mut best: Option<(&String, i64)> = None;
        for (w, &c) in cases {
            if best.is_none_or(|(_, bc)| c > bc) {
                best = Some((w, c));
            }
        }
        let first = best.map(|(w, _)| w.clone()).unwrap_or_default();
        out.counts.insert(first.clone(), cases.values().sum());
        out.standard_forms.insert(word_lower.clone(), first);
    }
    for (plural, singular) in merged_plurals {
        if let Some(std) = out.standard_forms.get(&singular.to_lowercase()).cloned() {
            out.standard_forms.insert(plural, std);
        }
    }
    out
}

/// Counts unigrams and adds statistically significant bigrams ("collocations").
///
/// Bigrams are formed *before* stopword removal (so "thank you very much" never
/// yields "thank much") and only kept when neither word is a stopword and the
/// Dunning score exceeds `collocation_threshold`. The two constituent unigram
/// counts are discounted by the bigram count and non-positive counts dropped.
pub fn unigrams_and_bigrams(
    words: &[String],
    stopwords: &HashSet<String>,
    normalize_plurals: bool,
    collocation_threshold: f64,
) -> Counts {
    let mut vocab: FxIndexSet<&str> = FxIndexSet::default();
    let seq: Vec<u32> = words
        .iter()
        .map(|w| vocab.insert_full(w.as_str()).0 as u32)
        .collect();
    let stop: Vec<bool> = vocab
        .iter()
        .map(|w| stopwords.contains(&w.to_lowercase()))
        .collect();
    let vocab: Vec<&str> = vocab.into_iter().collect();
    unigrams_and_bigrams_ids(
        &vocab,
        &seq,
        &stop,
        normalize_plurals,
        collocation_threshold,
    )
}

/// [`unigrams_and_bigrams`] on interned tokens: `seq` indexes `vocab`, and
/// `stop[i]` says whether `vocab[i]` is a stopword.
pub(crate) fn unigrams_and_bigrams_ids(
    vocab: &[&str],
    seq: &[u32],
    stop: &[bool],
    normalize_plurals: bool,
    collocation_threshold: f64,
) -> Counts {
    let is_stop = |id: u32| stop[id as usize];
    let mut bigram_ids: FxIndexMap<(u32, u32), i64> = FxIndexMap::default();
    for p in seq.windows(2) {
        if !is_stop(p[0]) && !is_stop(p[1]) {
            *bigram_ids.entry((p[0], p[1])).or_insert(0) += 1;
        }
    }
    let mut unigram_ids: FxIndexMap<u32, i64> = FxIndexMap::default();
    let mut n_words = 0i64;
    for &id in seq {
        if !is_stop(id) {
            *unigram_ids.entry(id).or_insert(0) += 1;
            n_words += 1;
        }
    }
    let uni = process_weighted(
        unigram_ids.iter().map(|(&id, &c)| (vocab[id as usize], c)),
        normalize_plurals,
    );
    let bi = process_weighted(
        bigram_ids
            .iter()
            .map(|(&(a, b), &c)| (format!("{} {}", vocab[a as usize], vocab[b as usize]), c)),
        normalize_plurals,
    );
    let orig_counts = uni.counts.clone();
    let mut counts = uni.counts;
    let standard = |w: &str| -> String {
        let lower = w.to_lowercase();
        uni.standard_forms
            .get(&lower)
            .cloned()
            .unwrap_or_else(|| w.to_string())
    };
    for (bigram, &count) in &bi.counts {
        let mut parts = bigram.split(' ');
        let word1 = standard(parts.next().unwrap_or(""));
        let word2 = standard(parts.next().unwrap_or(""));
        let c1 = orig_counts.get(&word1).copied().unwrap_or(0);
        let c2 = orig_counts.get(&word2).copied().unwrap_or(0);
        if score(count, c1, c2, n_words) > collocation_threshold {
            *counts.entry(word1).or_insert(0) -= count;
            *counts.entry(word2).or_insert(0) -= count;
            counts.insert(bigram.clone(), count);
        }
    }
    counts.retain(|_, c| *c > 0);
    counts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> {
        v.iter().map(|x| x.to_string()).collect()
    }

    #[test]
    fn most_common_case_wins_and_first_seen_breaks_ties() {
        let p = process_tokens(&s(&["Apple", "apple", "APPLE", "apple"]), false);
        assert_eq!(p.counts.get("apple"), Some(&4));
        let p = process_tokens(&s(&["Bee", "bee"]), false);
        assert!(p.counts.contains_key("Bee"));
    }

    #[test]
    fn ss_words_are_not_plurals() {
        let p = process_tokens(&s(&["glass", "glas"]), true);
        assert_eq!(p.counts.len(), 2);
    }

    #[test]
    fn plurals_disabled() {
        let p = process_tokens(&s(&["cats", "cat"]), false);
        assert_eq!(p.counts.len(), 2);
    }

    #[test]
    fn collocations_detected() {
        let mut words = Vec::new();
        for i in 0..40 {
            words.extend(s(&["Mock", "Turtle", "said"]));
            words.push(format!("filler{i}"));
        }
        let c = unigrams_and_bigrams(&words, &HashSet::new(), true, 30.0);
        assert_eq!(c.get("Mock Turtle"), Some(&40));
        assert!(!c.contains_key("Mock"));
    }

    #[test]
    fn drop_last_chars_unicode() {
        assert_eq!(drop_last_chars("héé", 1), "hé");
        assert_eq!(drop_last_chars("a", 2), "");
    }
}

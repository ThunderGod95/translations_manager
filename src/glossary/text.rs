use once_cell::sync::Lazy;
use regex::Regex;
use strsim::normalized_levenshtein;
use unicode_normalization::UnicodeNormalization;

/// This regex matches any character that is NOT a Han character,
/// punctuation, or a number. This includes all whitespace.
static RE_PREPROCESS: Lazy<Regex> = Lazy::new(|| Regex::new(r"[^\p{Han}\p{P}\p{N}]").unwrap());

pub fn preprocess_chinese_text(text: &str) -> String {
    let normalized = text.nfkc().collect::<String>();
    RE_PREPROCESS.replace_all(&normalized, "").into_owned()
}

pub fn calculate_similarity(s1: &str, s2: &str) -> i32 {
    (normalized_levenshtein(s1, s2) * 100.0) as i32
}

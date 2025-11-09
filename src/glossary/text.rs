use std::collections::HashSet;

use aho_corasick::AhoCorasick;
use jieba_rs::Jieba;
use once_cell::sync::Lazy;
use rayon::prelude::*;
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

pub fn aho_corasick_find_all(
    ac: &AhoCorasick,
    valid_clean_terms: &Vec<String>,
    clean_text: &str,
) -> HashSet<String> {
    let mut found_clean_terms = HashSet::new();

    for mat in ac.find_iter(clean_text) {
        let pattern_id = mat.pattern().as_usize();
        let clean_term = valid_clean_terms[pattern_id].clone();
        found_clean_terms.insert(clean_term);
    }
    found_clean_terms
}

pub fn chinese_fuzzy_search(
    jieba: &Jieba,
    terms: &[String],
    text: &str,
    threshold: Option<i32>,
) -> HashSet<String> {
    let threshold = threshold.unwrap_or(85);
    let text_words: HashSet<&str> = jieba.cut(text, true).iter().copied().collect();

    terms
        .par_iter()
        .filter_map(|clean_term| {
            let is_match = text_words
                .iter()
                .any(|&word| calculate_similarity(clean_term, word) >= threshold);

            if is_match {
                Some(clean_term.clone())
            } else {
                None
            }
        })
        .collect()
}

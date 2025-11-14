use std::{collections::HashSet, sync::LazyLock};

use aho_corasick::AhoCorasick;
use bk_tree::{BKTree, metrics::Levenshtein};
use jieba_rs::Jieba;
use rayon::prelude::*;
use regex::Regex;
use unicode_normalization::UnicodeNormalization;

/// This regex matches any character that is NOT a Han character,
/// punctuation, or a number. This includes all whitespace.
static RE_PREPROCESS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"[^\p{Han}\p{P}\p{N}]").unwrap());

pub fn preprocess_chinese_text<'a>(text: &str) -> String {
    let normalized = text.nfkc().collect::<String>();
    RE_PREPROCESS.replace_all(&normalized, "").to_string()
}

pub fn aho_corasick_find_all<'a>(
    ac: &AhoCorasick,
    terms: &'a Vec<String>,
    text: &str,
) -> HashSet<&'a str> {
    let mut found_clean_terms = HashSet::new();

    for mat in ac.find_iter(text) {
        let pattern_id = mat.pattern().as_usize();
        let clean_term = terms[pattern_id].as_str();
        found_clean_terms.insert(clean_term);
    }

    found_clean_terms
}

pub fn chinese_fuzzy_search<'a>(
    jieba: &Jieba,
    terms: &'a [&'a str],
    text: &'a str,
    threshold: Option<u32>,
) -> HashSet<&'a str> {
    let threshold = threshold.unwrap_or(1);
    let text_words: Vec<_> = jieba.cut(text, true).into_par_iter().collect();

    if text_words.is_empty() {
        return HashSet::new();
    }

    let mut bk_tree = BKTree::new(Levenshtein);
    bk_tree.extend(terms);

    text_words
        .par_iter()
        .flat_map(|word| {
            bk_tree
                .find(word, threshold)
                .map(|(_dist, term)| **term)
                .collect::<Vec<_>>()
        })
        .collect()
}

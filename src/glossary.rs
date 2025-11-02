use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    fs::{File, read_dir, read_to_string},
    io::BufReader,
    path::{Path, PathBuf},
    time::Instant,
};

use aho_corasick::AhoCorasick;
use anyhow::{Context, Result};
use clipboard_win::set_clipboard_string;
use itertools::Itertools;
use jieba_rs::Jieba;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use strsim::normalized_levenshtein;

use once_cell::sync::Lazy;
use regex::Regex;
use unicode_normalization::UnicodeNormalization;

use crate::{config::CONFIG, util::*};

#[derive(Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Clone)]
struct GlossaryEntry {
    en: String,
    cn: String,
    pinyin: String,
    #[serde(rename = "type")]
    _type: String,
    gender: Option<String>,
    file: Option<i32>,
}

#[derive(Debug, PartialEq, Eq)]
struct Chapter<'a> {
    expected_number: usize,
    original_number: usize,
    expected_title: String,
    original_title: String,
    text: Cow<'a, str>,
}

/// This regex matches any character that is NOT a Han character,
/// punctuation, or a number. This includes all whitespace.
static RE_PREPROCESS: Lazy<Regex> = Lazy::new(|| Regex::new(r"[^\p{Han}\p{P}\p{N}]").unwrap());
static RE_CHAPTER: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"(?ms)^第(\d+)章.*?\((本章完)\)\s*$").unwrap());

pub struct GlossaryProcessor {
    jieba: Jieba,
    glossary_data: Vec<GlossaryEntry>,
    original_to_clean_map: HashMap<String, String>,
    #[allow(dead_code)]
    clean_to_original_map: HashMap<String, String>,
    valid_clean_terms: Vec<String>,
    ac: AhoCorasick,
    assets_path: PathBuf,
    translations_path: PathBuf,
}

impl GlossaryProcessor {
    pub fn new(assets_path: impl AsRef<Path>, translations_path: impl AsRef<Path>) -> Result<Self> {
        let assets_path = assets_path.as_ref().to_owned();
        let translations_path = translations_path.as_ref().to_owned();

        log_info("Loading Chinese segmenter dictionary...");
        let jieba = Jieba::new();
        log_success("Dictionary loaded.");

        log_info("\nLoading glossary...");
        let glossary_data = read_glossary(assets_path.join(&CONFIG.glossary_file))
            .context("Failed to read glossary file")?;

        log_info("Preprocessing glossary...");
        let (original_to_clean_map, clean_to_original_map) = preprocess_glossary(&glossary_data);

        let valid_clean_terms: Vec<String> = clean_to_original_map.keys().cloned().collect();

        log_success(format!("Loaded {} glossary entries.", glossary_data.len()));
        log_success(format!(
            "{} are valid for searching.",
            valid_clean_terms.len()
        ));

        log_info("Building Aho-Corasick Engine...");
        let ac =
            AhoCorasick::new(&valid_clean_terms).context("Failed to build Aho-Corasick Engine")?;
        log_success("Engine built.");

        Ok(Self {
            jieba,
            glossary_data,
            original_to_clean_map,
            clean_to_original_map,
            valid_clean_terms,
            ac,
            assets_path,
            translations_path,
        })
    }

    pub fn process_new_chapters(&self) -> Result<()> {
        let chapter_file_path = self.assets_path.join(&CONFIG.chapter_file);
        let chapter_file = read_to_string(chapter_file_path).context(format!(
            "Failed to read chapter file: {}",
            &CONFIG.chapter_file
        ))?;

        log_info(format!(
            "\nScanning for chapters in {}",
            &CONFIG.chapter_file
        ));

        let last_chapter_number = get_last_chapter_number(&self.translations_path)
            .context("Failed to get last chapter number")?;
        let chapters = process_chapters(&chapter_file, last_chapter_number);

        if chapters.is_empty() {
            return Ok(());
        }

        let mut combined_chapter_text = String::new();
        for chapter in &chapters {
            combined_chapter_text.push_str(&chapter.text);
            combined_chapter_text.push_str("\n\n---\n\n");
        }
        let processed_chapter_text = preprocess_chinese_text(&combined_chapter_text);

        log_info("\n\n--- Starting Glossary Search (for all chapters) ---\n");
        let time = Instant::now();

        let exact_matches = self.aho_corasick_find_all(&processed_chapter_text);
        log_success(format!(
            "[{}ms] Phase 1 (Chinese Exact): Found {} unique terms.",
            time.elapsed().as_millis(),
            exact_matches.len()
        ));

        let terms_for_fuzzy_match: Vec<_> = self
            .valid_clean_terms
            .iter()
            .filter(|term| !exact_matches.contains(term.as_str()))
            .cloned()
            .collect();

        let fuzzy_matches = self.chinese_fuzzy_search(
            &terms_for_fuzzy_match,
            &processed_chapter_text,
            Some(CONFIG.fuzzy_search_threshold),
        );
        log_success(format!(
            "[{}ms] Phase 2 (Chinese Fuzzy): Found {} unique terms.",
            time.elapsed().as_millis(),
            fuzzy_matches.len()
        ));

        let terms_for_ngram_match: Vec<_> = self
            .valid_clean_terms
            .iter()
            .filter(|term| !fuzzy_matches.contains(term.as_str()))
            .cloned()
            .collect();

        let ngram_matches = self.chinese_ngram_search(
            &terms_for_ngram_match,
            &processed_chapter_text,
            CONFIG.ngram_search_max_length,
        );
        log_success(format!(
            "[{}ms] Phase 3 (Chinese N-gram/Subsequence): Found {} unique terms.",
            time.elapsed().as_millis(),
            ngram_matches.len()
        ));

        let mut all_found_terms = HashSet::new();
        all_found_terms.extend(exact_matches);
        all_found_terms.extend(fuzzy_matches);
        all_found_terms.extend(ngram_matches);

        log_info(format!(
            "\nTotal unique glossary terms after all phases: {}.",
            all_found_terms.len()
        ));

        let found_entries: Vec<_> = self
            .glossary_data
            .iter()
            .filter_map(|entry| {
                let clean_term = self.original_to_clean_map.get(&entry.cn)?;
                if all_found_terms.contains(clean_term) {
                    Some(entry.clone())
                } else {
                    None
                }
            })
            .collect();

        log_info("\n--- Final Micro-Glossary Terms ---\n");
        for entry in &found_entries {
            println!("* {} - {}", entry.cn, entry.en);
        }

        let micro_glossary_string = found_entries
            .iter()
            .map(|entry| {
                let cn = &entry.cn;
                let pinyin = &entry.pinyin;
                let en = &entry.en;

                let mut details = vec![&entry._type];
                if let Some(gender) = &entry.gender {
                    details.push(gender);
                }
                let details_string = format!("[{}]", details.iter().join(", "));

                format!("* {} ({}) -> {} {}", cn, pinyin, en, details_string)
            })
            .join("\n");

        let prompt_template =
            read_to_string(self.assets_path.join(&CONFIG.translation_prompt_file))
                .context("Failed to read translation prompt file")?;

        let final_prompt_string = format!(
            "{}\n\n**Glossary**\n\n{}\n\n---\n\n**Chinese Chapter(s) to Translate:**\n{}",
            prompt_template, micro_glossary_string, combined_chapter_text
        );

        set_clipboard_string(&final_prompt_string)
            .map_err(|e| anyhow::anyhow!("Failed to set clipboard: {}", e))?;

        let new_files: Vec<_> = chapters
            .iter()
            .map(|chapter| {
                self.translations_path
                    .join(format!("{}{}", chapter.expected_number, ".md"))
            })
            .collect();

        println!();
        create_and_open_files(&new_files);

        let separator = "=".repeat(50);
        println!("\n{}", separator);
        log_success(format!(
            "✅ SUCCESS! The prompt for {} chapter(s) has been copied to clipboard.",
            chapters.len()
        ));
        log_success(format!(
            "✅ Total glossary entries: {}",
            found_entries.len()
        ));
        log_success(format!("✅ Created/Verified {} file(s)", new_files.len()));
        println!("{}", separator);

        Ok(())
    }

    fn aho_corasick_find_all(&self, clean_text: &str) -> HashSet<String> {
        let mut found_clean_terms = HashSet::new();

        for mat in self.ac.find_iter(clean_text) {
            let pattern_id = mat.pattern().as_usize();
            let clean_term = self.valid_clean_terms[pattern_id].clone();
            found_clean_terms.insert(clean_term);
        }
        found_clean_terms
    }

    fn chinese_fuzzy_search(
        &self,
        terms: &[String],
        text: &str,
        threshold: Option<i32>,
    ) -> HashSet<String> {
        let threshold = threshold.unwrap_or(85);
        let text_words: HashSet<&str> = self.jieba.cut(text, true).iter().copied().collect();

        terms
            .par_iter()
            .filter_map(|original_term| {
                let clean_term = match self.original_to_clean_map.get(original_term) {
                    Some(term) => term,
                    None => return None,
                };

                let is_match = text_words
                    .iter()
                    .any(|&word| calculate_similarity(clean_term, word) >= threshold);

                if is_match {
                    Some(original_term.clone())
                } else {
                    None
                }
            })
            .collect()
    }

    fn chinese_ngram_search(
        &self,
        terms: &[String],
        text: &str,
        ngram_search_max_length: usize,
    ) -> HashSet<String> {
        terms
            .par_iter()
            .filter_map(|original_term| {
                let clean_term = match self.original_to_clean_map.get(original_term) {
                    Some(t) if t.is_empty() => return None,
                    Some(t) => t.as_str(),
                    None => return None,
                };

                let char_count = clean_term.chars().count();

                if char_count > 0 && char_count <= ngram_search_max_length {
                    if is_subsequence(text, clean_term) {
                        return Some(original_term.clone());
                    }
                }

                None
            })
            .collect()
    }
}

fn preprocess_chinese_text(text: &str) -> String {
    let normalized = text.nfkc().collect::<String>();
    RE_PREPROCESS.replace_all(&normalized, "").into_owned()
}

fn calculate_similarity(s1: &str, s2: &str) -> i32 {
    (normalized_levenshtein(s1, s2) * 100.0) as i32
}

fn is_subsequence(text: &str, term: &str) -> bool {
    let mut text_chars = text.chars();
    for term_char in term.chars() {
        if text_chars
            .find(|&text_char| text_char == term_char)
            .is_none()
        {
            return false;
        }
    }
    true
}

fn get_last_chapter_number(translations_path: impl AsRef<Path>) -> Result<usize> {
    let entries = read_dir(translations_path)?;
    let max_chapter = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let file_name = entry.file_name();
            let name_str = file_name.to_str()?;
            if !name_str.ends_with(".md") {
                return None;
            }
            let end_of_num = name_str
                .find(|c: char| !c.is_ascii_digit())
                .unwrap_or(name_str.len());
            let num_str = &name_str[..end_of_num];
            num_str.parse::<usize>().ok()
        })
        .max()
        .unwrap_or(0);
    Ok(max_chapter)
}

fn read_glossary(glossary_path: impl AsRef<Path>) -> Result<Vec<GlossaryEntry>> {
    let file = File::open(glossary_path)?;
    let reader = BufReader::new(file);
    let glossary_data: Vec<GlossaryEntry> =
        serde_json::from_reader(reader).context("Failed to parse glossary JSON")?;
    Ok(glossary_data)
}

fn preprocess_glossary(
    glossary_data: &[GlossaryEntry],
) -> (HashMap<String, String>, HashMap<String, String>) {
    let capacity = glossary_data.len();
    let mut original_to_clean = HashMap::with_capacity(capacity);
    let mut clean_to_original = HashMap::with_capacity(capacity);

    for entry in glossary_data {
        let clean_term = preprocess_chinese_text(&entry.cn);
        if !clean_term.is_empty() {
            original_to_clean.insert(entry.cn.clone(), clean_term.clone());
            clean_to_original.insert(clean_term, entry.cn.clone());
        }
    }
    (original_to_clean, clean_to_original)
}

fn process_chapters<'a>(cr_ch_text: &'a str, last_chapter_number: usize) -> Vec<Chapter<'a>> {
    let captures: Vec<_> = RE_CHAPTER.captures_iter(cr_ch_text).collect();

    if captures.is_empty() {
        log_error(
            "No chapters found. A chapter must start with '第...章' and end with '(本章完)' on its own line.",
        );
        return vec![];
    }

    log_info(format!("Found {} chapter(s).", captures.len()));

    let mut expected_chapter_number = last_chapter_number + 1;
    let mut processed_chapters: Vec<Chapter<'a>> = Vec::with_capacity(captures.len());

    for cap in captures {
        let Some(full_match) = cap.get(0) else {
            continue;
        };
        let original_chapter_text: &'a str = full_match.as_str();

        let Some(num_match) = cap.get(1) else {
            log_warning("⚠️ Found chapter match but no chapter number capture group. Skipping...");
            continue;
        };

        let Ok(actual_chapter_number) = num_match.as_str().parse::<usize>() else {
            log_warning(format!(
                "⚠️ Could not parse chapter number from: '{}'. Skipping...",
                num_match.as_str()
            ));
            continue;
        };

        log_info(format!(
            "\nProcessing found chapter {} (expected {})...",
            actual_chapter_number, expected_chapter_number
        ));

        let original_title = format!("第{}章", actual_chapter_number);

        let (current_chapter_number, expected_title, chapter_text_cow) = if actual_chapter_number
            != expected_chapter_number
        {
            log_warning(format!(
                "⚠️ Chapter number mismatch. Found {}, expected {}.",
                actual_chapter_number, expected_chapter_number
            ));

            let expected_title = format!("第{}章", expected_chapter_number);

            let modified_text = original_chapter_text.replacen(&original_title, &expected_title, 1);

            log_info("✅ Chapter number corrected in text.");

            (
                expected_chapter_number,
                expected_title,
                Cow::Owned(modified_text),
            )
        } else {
            log_info("✅ Chapter number is correct.");

            (
                actual_chapter_number,
                original_title.clone(),
                Cow::Borrowed(original_chapter_text),
            )
        };

        processed_chapters.push(Chapter {
            expected_number: current_chapter_number,
            original_number: actual_chapter_number,
            expected_title,
            original_title,
            text: chapter_text_cow,
        });

        expected_chapter_number += 1;
    }

    processed_chapters
}

fn create_and_open_files(paths: &[impl AsRef<Path>]) {
    let mut paths_to_open: Vec<&Path> = Vec::with_capacity(paths.len());

    for path_ref in paths {
        let path = path_ref.as_ref();

        match File::create(path) {
            Ok(_) => {
                paths_to_open.push(path);
            }
            Err(e) => {
                log_error(format!(
                    "Error: Failed to create/overwrite file {}: {}",
                    path.display(),
                    e
                ));
            }
        }
    }

    if !paths_to_open.is_empty() {
        open_in_vs_code(&paths_to_open);
    }
}

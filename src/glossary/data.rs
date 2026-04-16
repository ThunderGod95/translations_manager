use std::{collections::HashMap, path::Path};

use anyhow::{Context, Result};
use itertools::Itertools;
use std::fs;

use super::{GlossaryEntry, text::preprocess_chinese_text};

/// Reads and parses the JSON glossary file.
pub fn read_glossary(glossary_path: impl AsRef<Path>) -> Result<Vec<GlossaryEntry>> {
    let mut file_content = fs::read(glossary_path).context("Failed to read glossary file")?;

    let glossary_data: Vec<GlossaryEntry> =
        simd_json::from_slice(&mut file_content).context("Failed to parse glossary JSON")?;

    Ok(glossary_data)
}

/// Creates lookup maps from the glossary data.
pub fn preprocess_glossary(
    glossary_data: &[GlossaryEntry],
) -> (HashMap<String, String>, Vec<String>) {
    let capacity = glossary_data.len();
    let mut original_to_clean = HashMap::with_capacity(capacity);
    let mut clean_to_original = Vec::with_capacity(capacity);

    for entry in glossary_data {
        let clean_term = preprocess_chinese_text(&entry.cn);
        if !clean_term.is_empty() {
            original_to_clean.insert(entry.cn.clone(), clean_term.clone());
            clean_to_original.push(clean_term);
        }
    }
    (original_to_clean, clean_to_original)
}

pub fn create_micro_glossary(found_entries: &Vec<&GlossaryEntry>) -> String {
    found_entries
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

            let mut line = format!("* {} ({}) -> {} {}", cn, pinyin, en, details_string);

            if let Some(summary) = &entry.summary {
                line.push_str(&format!(" - {}", summary));
            }

            line
        })
        .join("\n")
}

use std::{collections::HashMap, fs::File, io::BufReader, path::Path};

use anyhow::{Context, Result};

use super::{text::preprocess_chinese_text, types::GlossaryEntry};

/// Reads and parses the JSON glossary file.
pub fn read_glossary(glossary_path: impl AsRef<Path>) -> Result<Vec<GlossaryEntry>> {
    let file = File::open(glossary_path).context("Failed to open glossary file")?;
    let reader = BufReader::new(file);
    let glossary_data: Vec<GlossaryEntry> =
        serde_json::from_reader(reader).context("Failed to parse glossary JSON")?;
    Ok(glossary_data)
}

/// Creates lookup maps from the glossary data.
pub fn preprocess_glossary(
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

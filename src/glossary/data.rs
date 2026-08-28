use std::{fs, path::Path};

use anyhow::{Context, Result};
use itertools::Itertools;

use crate::core::glossary::GlossaryEntry;

/// Reads and parses the project's JSON glossary file.
pub fn read_glossary(glossary_path: impl AsRef<Path>) -> Result<Vec<GlossaryEntry>> {
    let mut file_content = fs::read(glossary_path).context("Failed to read glossary file")?;

    let glossary_data: Vec<GlossaryEntry> =
        simd_json::from_slice(&mut file_content).context("Failed to parse glossary JSON")?;

    Ok(glossary_data)
}

/// Formats structured glossary entries for the translation prompt.
pub fn format_micro_glossary(found_entries: &[GlossaryEntry]) -> String {
    found_entries
        .iter()
        .map(|entry| {
            let mut details = vec![entry._type.as_str()];

            if let Some(gender) = entry.gender.as_deref() {
                details.push(gender);
            }

            let details_string = format!("[{}]", details.into_iter().join(", "));

            let mut line = format!(
                "* {} ({}) -> {} {}",
                entry.cn, entry.pinyin, entry.en, details_string
            );

            if let Some(summary) = &entry.summary {
                line.push_str(&format!(" - {}", summary));
            }

            line
        })
        .join("\n")
}

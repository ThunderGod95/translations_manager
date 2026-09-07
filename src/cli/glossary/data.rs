use std::{fs, path::Path};

use anyhow::{Context, Result};

use crate::core::glossary::GlossaryEntry;

/// Reads and parses the project's JSON glossary file.
pub fn read_glossary(glossary_path: impl AsRef<Path>) -> Result<Vec<GlossaryEntry>> {
    let mut file_content = fs::read(glossary_path).context("Failed to read glossary file")?;

    let glossary_data: Vec<GlossaryEntry> =
        simd_json::from_slice(&mut file_content).context("Failed to parse glossary JSON")?;

    Ok(glossary_data)
}

use anyhow::Result;
use itertools::Itertools;
use serde::{Deserialize, Serialize};

use super::glossary::{GlossaryEntry, GlossaryOptions, create_micro_glossary_with_options};

#[derive(Debug, Serialize, Deserialize)]
pub struct PreparePromptRequest {
    pub chapter: String,
    pub glossary: Vec<GlossaryEntry>,
    pub translation_prompt: String,

    #[serde(default, rename = "options")]
    pub glossary_options: GlossaryOptions,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PreparePromptResult {
    pub prompt: String,
    pub micro_glossary: Vec<GlossaryEntry>,
}

pub fn prepare_translation_prompt(request: PreparePromptRequest) -> Result<PreparePromptResult> {
    let micro_glossary = create_micro_glossary_with_options(
        &request.chapter,
        &request.glossary,
        request.glossary_options,
    )?;

    let formatted_glossary = format_micro_glossary(&micro_glossary);

    let prompt = build_translation_prompt(
        &request.translation_prompt,
        &formatted_glossary,
        &request.chapter,
    );

    Ok(PreparePromptResult {
        prompt,
        micro_glossary,
    })
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

pub fn build_translation_prompt(
    prompt_template: &str,
    micro_glossary_string: &str,
    combined_chapter_text: &str,
) -> String {
    format!(
        "{}\n\n\
             **Glossary**\n\n\
             {}\n\n\
             ---\n\n\
             **Chinese Chapter(s) to Translate:**\n\
             {}",
        prompt_template, micro_glossary_string, combined_chapter_text
    )
}

use std::{fs::read_dir, path::Path};

use anyhow::Result;
use log::{error, info, warn};
use once_cell::sync::Lazy;
use regex::Regex;

use super::Chapter;

static RE_SPLITTER: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?m)^第").unwrap());
static RE_PARSER: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?s)^(\d+)章([^\n\r]*)(.*)").unwrap());

pub fn process_chapters<'a>(cr_ch_text: &'a str, last_chapter_number: usize) -> Vec<Chapter> {
    let chunks: Vec<_> = RE_SPLITTER.split(cr_ch_text.trim()).skip(1).collect();

    if chunks.is_empty() {
        warn!("No chapters found. A chapter must start with '第...章'.");
        return vec![];
    }

    info!("Found {} potential chapter(s).", chunks.len());

    let mut expected_chapter_number = last_chapter_number + 1;
    let mut processed_chapters: Vec<Chapter> = Vec::with_capacity(chunks.len());

    for chunk in chunks.into_iter() {
        let Some(cap) = RE_PARSER.captures(chunk) else {
            warn!("Failed to parse chapter chunk. Skipping...");
            continue;
        };

        let Some(num_match) = cap.get(1) else {
            continue;
        };

        let Ok(actual_chapter_number) = num_match.as_str().parse::<usize>() else {
            let first_line = chunk.lines().next().unwrap_or("").trim();
            let preview: String = first_line.chars().take(150).collect();

            error!(
                "Failed to parse chapter chunk. Skipping... Chunk starts with: '{}...'",
                preview
            );
            continue;
        };

        let title_text = cap.get(2).map_or("", |m| m.as_str().trim());
        let original_content = cap.get(3).map_or("", |m| m.as_str());

        info!(
            "Processing chapter {} (expected {}): {}",
            actual_chapter_number, expected_chapter_number, title_text
        );

        let original_title = format!("第{}章 {}", actual_chapter_number, title_text)
            .trim_end()
            .to_string();

        let (current_chapter_number, expected_title) =
            if actual_chapter_number != expected_chapter_number {
                warn!(
                    "Chapter number mismatch. Found {}, expected {}.",
                    actual_chapter_number, expected_chapter_number
                );

                let expected_title = format!("第{}章 {}", expected_chapter_number, title_text)
                    .trim_end()
                    .to_string();

                (expected_chapter_number, expected_title)
            } else {
                (actual_chapter_number, original_title.clone())
            };

        let chapter_text = format!("{}\n{}", expected_title, original_content);

        processed_chapters.push(Chapter {
            expected_number: current_chapter_number,
            original_number: actual_chapter_number,
            expected_title,
            original_title,
            text: chapter_text,
        });

        expected_chapter_number = current_chapter_number + 1;
    }

    processed_chapters
}

/// Finds the highest chapter number in the translations directory.
pub fn get_last_chapter_number(translations_path: impl AsRef<Path>) -> Result<usize> {
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

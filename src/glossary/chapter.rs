use std::{fs::read_dir, path::Path};

use anyhow::Result;
use log::warn;
use once_cell::sync::Lazy;
use regex::Regex;

use super::Chapter;

static RE_SPLITTER: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?m)^第").unwrap());
static RE_PARSER: Lazy<Regex> = Lazy::new(|| Regex::new(r"(?s)^.+?章([^\n\r]*)(.*)").unwrap());

pub fn process_chapters<'a>(cr_ch_text: &'a str, last_chapter_number: usize) -> Vec<Chapter> {
    let chunks: Vec<_> = RE_SPLITTER.split(cr_ch_text.trim()).skip(1).collect();

    if chunks.is_empty() {
        warn!("No chapters found. A chapter must start with '第...章'.");
        return vec![];
    }

    println!("Found {} potential chapter(s).", chunks.len());

    let mut expected_number = last_chapter_number + 1;
    let mut processed_chapters: Vec<Chapter> = Vec::with_capacity(chunks.len());

    for chunk in chunks.into_iter() {
        let Some(cap) = RE_PARSER.captures(chunk) else {
            warn!("Failed to parse chapter chunk. Skipping...");
            continue;
        };

        let title_text = cap.get(1).map_or("", |m| m.as_str().trim());
        let original_content = cap.get(2).map_or("", |m| m.as_str());

        let expected_title = format!("第{}章 {}", expected_number, title_text)
            .trim_end()
            .to_string();

        let chapter_text = format!("{}\n{}", expected_title, original_content);

        processed_chapters.push(Chapter {
            expected_number,
            expected_title,
            text: chapter_text,
        });

        expected_number += 1;
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

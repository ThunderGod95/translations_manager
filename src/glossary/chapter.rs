use std::{borrow::Cow, fs::read_dir, path::Path};

use anyhow::Result;
use log::{info, warn};
use once_cell::sync::Lazy;
use regex::Regex;

use super::Chapter;

static RE_CHAPTER: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        // Group 1: Number (e.g., "1310")
        // Group 2: Title text (e.g., " 感言及中奖编号")
        // Group 3: Content (e.g., "\n\n　　大家晚上好...")
        r"(?ms)^第(\d+)章([^\n\r]*)(\n.*)?(?=^第\d+章|\z)",
    )
    .unwrap()
});

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

/// Parses the raw chapter text, extracts chapters, and corrects their numbering.
pub fn process_chapters<'a>(cr_ch_text: &'a str, last_chapter_number: usize) -> Vec<Chapter<'a>> {
    let captures: Vec<_> = RE_CHAPTER.captures_iter(cr_ch_text.trim()).collect();

    if captures.is_empty() {
        warn!("No chapters found. A chapter must start with '第...章'.",);
        return vec![];
    }

    info!("Found {} potential chapter(s).", captures.len());

    let mut expected_chapter_number = last_chapter_number + 1;
    let mut processed_chapters: Vec<Chapter<'a>> = Vec::with_capacity(captures.len());

    for cap in captures.into_iter() {
        if let Some(chapter) = process_single_chapter(cap, expected_chapter_number) {
            expected_chapter_number = chapter.expected_number + 1;
            processed_chapters.push(chapter);
        }
    }

    processed_chapters
}

/// Processes regex capture into a Chapter struct.
fn process_single_chapter<'a>(
    cap: regex::Captures<'a>,
    expected_chapter_number: usize,
) -> Option<Chapter<'a>> {
    let Some(num_match) = cap.get(1) else {
        warn!("Found chapter but no chapter number. Skipping...");
        return None;
    };
    let Ok(actual_chapter_number) = num_match.as_str().parse::<usize>() else {
        warn!(
            "Could not parse chapter number from: '{}'. Skipping...",
            num_match.as_str()
        );
        return None;
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

    let (current_chapter_number, expected_title, chapter_text_cow) = correct_chapter_numbering(
        original_content,
        title_text,
        actual_chapter_number,
        expected_chapter_number,
    );

    Some(Chapter {
        expected_number: current_chapter_number,
        original_number: actual_chapter_number,
        expected_title,
        original_title,
        text: chapter_text_cow,
    })
}

/// Corrects the chapter number and constructs the final title.
///
/// Returns a tuple containing:
/// 1. The number to be used for the chapter.
/// 2. The full title string to be used for the chapter.
/// 3. A Cow<'a, str> containing the chapter content.
fn correct_chapter_numbering<'a>(
    original_content: &'a str,
    title_text: &str,
    actual_chapter_number: usize,
    expected_chapter_number: usize,
) -> (usize, String, Cow<'a, str>) {
    let (current_chapter_number, final_title) = if actual_chapter_number != expected_chapter_number
    {
        warn!(
            "Chapter number mismatch. Found {}, expected {}.",
            actual_chapter_number, expected_chapter_number
        );

        let expected_title = format!("第{}章 {}", expected_chapter_number, title_text)
            .trim_end()
            .to_string();

        (expected_chapter_number, expected_title)
    } else {
        let original_title = format!("第{}章 {}", actual_chapter_number, title_text)
            .trim_end()
            .to_string();

        (actual_chapter_number, original_title)
    };

    (
        current_chapter_number,
        final_title,
        Cow::Borrowed(original_content),
    )
}

use std::{path::Path, sync::LazyLock};

use anyhow::{Context, Result, bail};
use console::style;
use jwalk::WalkDir;
use regex::Regex;

use super::Chapter;

static RE_SPLITTER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^第").unwrap());
static RE_PARSER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)^.+?章([^\n\r]*)(.*)").unwrap());

pub fn process_chapters<'a>(cr_ch_text: &'a str, last_chapter_number: usize) -> Vec<Chapter> {
    let chunks: Vec<_> = RE_SPLITTER.split(cr_ch_text.trim()).skip(1).collect();

    if chunks.is_empty() {
        println!(
            "{}",
            style(format!(
                "[WARN] No chapters found. A chapter must start with '第...章'."
            ))
            .yellow()
        );
        return vec![];
    }

    println!("Found {} potential chapter(s).", chunks.len());

    let mut expected_number = last_chapter_number + 1;
    let mut processed_chapters: Vec<Chapter> = Vec::with_capacity(chunks.len());

    for chunk in chunks.into_iter() {
        let Some(cap) = RE_PARSER.captures(chunk) else {
            println!(
                "{}",
                style(format!("[WARN] Failed to parse chapter chunk. Skipping...")).yellow()
            );
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

pub fn find_last_chapter(translations_path: impl AsRef<Path>) -> Result<usize> {
    let translations_path = translations_path.as_ref();

    if !translations_path.exists() {
        bail!(
            "The folder '{}' does not exist. Please check the path.",
            translations_path.display()
        );
    }

    if !translations_path.is_dir() {
        bail!(
            "The path '{}' is a file, not a folder. Please provide a path to a folder.",
            translations_path.display()
        );
    }

    let mut max_chapter = 0;

    let walker = WalkDir::new(&translations_path)
        .min_depth(1)
        .max_depth(1)
        .skip_hidden(true);

    for entry in walker.into_iter() {
        let entry = entry.with_context(|| {
            format!(
                "Could not read the files in folder '{}'. Do you have permission to open it?",
                translations_path.display()
            )
        })?;

        let file_name = entry.file_name();

        let name_str = match file_name.to_str() {
            Some(s) => s,
            None => continue,
        };

        if !name_str.ends_with(".md") {
            continue;
        }

        let end_of_num = name_str
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(name_str.len());

        if end_of_num == 0 {
            continue;
        }

        let num_str = &name_str[..end_of_num];

        if let Ok(num) = num_str.parse::<usize>() {
            max_chapter = max_chapter.max(num);
        }
    }

    Ok(max_chapter)
}

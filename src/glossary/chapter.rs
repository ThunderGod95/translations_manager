use std::{fs, path::Path, sync::LazyLock};

use anyhow::{Context, Result, bail};
use console::style;
use dialoguer::{Confirm, theme::ColorfulTheme};
use fancy_regex::Regex;

use super::Chapter;
static RE_SPLITTER: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^第").unwrap());
static RE_PARSER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[^\n\r]+?章([^\n\r]*)(?s)(.*)").unwrap());

pub fn process_chapters<'a>(
    cr_ch_text: &'a str,
    last_chapter_number: usize,
) -> Result<Vec<Chapter>> {
    if cr_ch_text.trim().is_empty() {
        bail!("Chapter file is empty.")
    }

    let chunks: Vec<_> = RE_SPLITTER.split(cr_ch_text.trim()).skip(1).collect();

    if chunks.is_empty() {
        return process_no_chapter(cr_ch_text);
    }

    println!("Found {} potential chapter segment(s).", chunks.len());

    let mut expected_number = last_chapter_number + 1;
    let mut processed_chapters: Vec<Chapter> = Vec::with_capacity(chunks.len());

    for chunk in chunks.into_iter() {
        let chunk = chunk?;

        if let Some(cap) = RE_PARSER.captures(chunk)? {
            let title_text = cap.get(1).map_or("", |m| m.as_str().trim());
            let original_content = cap.get(2).map_or("", |m| m.as_str());

            let expected_title = format!("第{}章 {}", expected_number, title_text)
                .trim_end()
                .to_string();

            let chapter_text = format!("{}{}", expected_title, original_content);

            processed_chapters.push(Chapter {
                expected_number,
                expected_title,
                text: chapter_text,
                create_file: true,
            });

            expected_number += 1;
        } else {
            if let Some(last_chapter) = processed_chapters.last_mut() {
                println!(
                    "{}",
                    style(format!(
                        "[WARN] Found line starting with '第' that is not a valid chapter heading.\n\tAppending text to previous chapter: '第{}...'",
                        chunk.chars().take(10).collect::<String>()
                    )).yellow()
                );

                last_chapter.text.push_str(&format!("\n第{}", chunk));
            } else {
                return process_no_chapter(cr_ch_text);
            }
        }
    }

    Ok(processed_chapters)
}

fn process_no_chapter(text: &str) -> Result<Vec<Chapter>> {
    println!("No chapters found. A chapter must start with '第...章'.");

    let confirm = Confirm::with_theme(&ColorfulTheme::default())
        .with_prompt("Do you still want to continue?")
        .default(false)
        .show_default(true)
        .interact()?;

    if !confirm {
        bail!("Task cancelled.")
    }

    return Ok(vec![Chapter {
        expected_number: 0,
        expected_title: "".to_string(),
        text: text.to_string(),
        create_file: false,
    }]);
}

pub fn find_last_chapter(translations_path: impl AsRef<Path>) -> Result<usize> {
    let translations_path = translations_path.as_ref();

    if !translations_path.exists() || !translations_path.is_dir() {
        bail!(
            "Path '{}' is not a valid directory.",
            translations_path.display()
        );
    }

    let mut max_chapter = 0;

    let entries = fs::read_dir(translations_path).with_context(|| {
        format!(
            "Could not read directory '{}'. Check permissions.",
            translations_path.display()
        )
    })?;

    for entry in entries {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };

        if let Ok(ft) = entry.file_type() {
            if !ft.is_file() {
                continue;
            }
        }

        let file_name = entry.file_name();

        let name_str = match file_name.to_str() {
            Some(s) => s,
            None => continue,
        };

        let bytes = name_str.as_bytes();

        if bytes.len() < 3 || &bytes[bytes.len() - 3..] != b".md" {
            continue;
        }

        let end_of_num = bytes
            .iter()
            .position(|&b| !b.is_ascii_digit())
            .unwrap_or(bytes.len());

        if end_of_num == 0 {
            continue;
        }

        if let Ok(num) = name_str[..end_of_num].parse::<usize>() {
            if num > max_chapter {
                max_chapter = num;
            }
        }
    }

    Ok(max_chapter)
}

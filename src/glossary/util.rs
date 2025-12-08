use anyhow::{Result, bail};
use arboard::Clipboard;
use console::style;
use std::fs::{self, File};
use std::path::Path;

use super::Chapter;
use crate::editor;

pub fn create_and_open_files(base_path: &Path, chapters: &[Chapter]) -> Result<()> {
    let new_files: Vec<_> = chapters
        .iter()
        .filter(|chapter| chapter.create_file)
        .map(|chapter| base_path.join(format!("{}{}", chapter.expected_number, ".md")))
        .collect();

    let mut paths_to_open: Vec<&Path> = Vec::with_capacity(new_files.len());

    for path in &new_files {
        match File::create(&path) {
            Ok(_) => {
                paths_to_open.push(&path);
            }
            Err(e) => {
                bail!("Failed to create/overwrite file {}: {}", path.display(), e);
            }
        }
    }

    if !paths_to_open.is_empty() {
        editor::open_in_editor(&paths_to_open)?;
    }

    println!("Created/Verified {} file(s)", new_files.len());

    Ok(())
}

/// Writes a list of raw untranslated chapter contents into the
/// target directory.
///
/// This function will automatically create the directory if it does not
/// exist.
///
/// If a write operation fails for a specific chapter, a warning
/// is printed to stdout, but the function will continue processing the remaining chapters.
pub fn write_raws(write_path: &Path, raws: &[Chapter]) {
    if let Err(e) = fs::create_dir_all(write_path) {
        println!(
            "{}",
            style(format!("[ERROR] Failed to create directory: {}", e)).red()
        );
    }

    for raw in raws {
        if !raw.create_file {
            continue;
        }

        let path = write_path.join(format!("{}.md", raw.expected_number));

        if let Err(e) = fs::write(path, &raw.text) {
            println!(
                "{}",
                style(format!(
                    "[WARN] Failed to save raw text for: Ch. {}. Reason: {}",
                    raw.expected_number, e
                ))
                .yellow()
            );
        }
    }
}

pub fn paste_glossary(content: &str) -> Result<()> {
    let mut clipboard = Clipboard::new()?;

    clipboard.set_text(content)?;

    #[cfg(target_os = "linux")]
    {
        println!("Prompt copied to clipboard!");
        println!("You can now paste the content into your target application.");
        println!("Press Ctrl+C to exit.");

        clipboard.wait()?;
    }

    Ok(())
}

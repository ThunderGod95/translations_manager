use anyhow::{Result, bail};
use arboard::Clipboard;
use std::fs::File;
use std::path::Path;

use super::Chapter;
use crate::util::open_in_vs_code;

pub fn create_and_open_files(base_path: &Path, chapters: &Vec<Chapter>) -> Result<()> {
    let new_files: Vec<_> = chapters
        .iter()
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
        open_in_vs_code(&paths_to_open);
    }

    println!("Created/Verified {} file(s)", new_files.len());

    Ok(())
}

pub fn paste_glossary(content: String) -> Result<()> {
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

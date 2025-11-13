use anyhow::{Result, bail};
use std::path::Path;
use tokio::fs::File;

use super::Chapter;
use crate::util::open_in_vs_code;

pub async fn create_and_open_files(base_path: &Path, chapters: &Vec<Chapter>) -> Result<()> {
    let new_files: Vec<_> = chapters
        .iter()
        .map(|chapter| base_path.join(format!("{}{}", chapter.expected_number, ".md")))
        .collect();

    let mut paths_to_open: Vec<&Path> = Vec::with_capacity(new_files.len());

    for path in &new_files {
        match File::create(&path).await {
            Ok(_) => {
                paths_to_open.push(&path);
            }
            Err(e) => {
                bail!("Failed to create/overwrite file {}: {}", path.display(), e);
            }
        }
    }

    if !paths_to_open.is_empty() {
        open_in_vs_code(&paths_to_open).await;
    }

    println!("Created/Verified {} file(s)", new_files.len());

    Ok(())
}

// pub async fn check_if_chapter_file_empty(translations_path: &Path, chapter: usize) {
//     let chapter_file_name = format!("{}.md", chapter);

// }

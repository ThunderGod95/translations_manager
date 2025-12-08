use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, bail};

use super::Chapter;
use crate::config::CONFIG;

pub fn get_translations_dir(project: &Path) -> Result<PathBuf> {
    let config = CONFIG.read().unwrap();

    let translations_path = project.join(&config.translations_folder);

    if !translations_path.exists() {
        bail!(
            "`{}` directory not found in project: {}",
            &config.translations_folder.display(),
            project.display()
        );
    }

    return Ok(translations_path);
}

pub fn write_clean_chapters(write_path: &Path, chapters: &[Chapter]) -> Result<()> {
    for chapter in chapters {
        if let Some(chap_num) = chapter.number_as_usize() {
            let file_path = write_path.join(format!("{}.md", chap_num));

            let content = format!(
                "# Chapter {}: {}\n\n{}\n\n",
                chapter.number, chapter.title, chapter.content
            );

            fs::write(&file_path, content.trim_end())
                .with_context(|| format!("Failed to write file: {}", file_path.display()))?;
        }
    }

    Ok(())
}

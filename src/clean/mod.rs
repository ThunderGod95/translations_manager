mod chapter;
pub mod navigation;
pub mod write;

use std::fs;
use std::path::Path;

use anyhow::{Context, Result, bail};

use crate::{
    config::CONFIG,
    util::{backup, collect_numbered_file_paths},
};
use chapter::*;
use write::*;

pub fn clean_project(project: &str, yaml: bool) -> Result<()> {
    let translations_path = &CONFIG.read().unwrap().get_translations_dir(project);
    let (paths, _) = collect_numbered_file_paths(&translations_path, Some("md"), None, None)?;

    println!(
        "\nTotal no. of translated chapters found: {}\n",
        paths.len()
    );

    backup(&translations_path)?;

    let mut all_chapters = Vec::new();

    for path in &paths {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read file: {}", path.display()))?;
        all_chapters.extend(parse_chapters(&content));
    }

    all_chapters.retain(|c| {
        let is_valid = c.number_as_u32().is_some();
        if !is_valid {
            eprintln!(
                "[WARN]: Skipping chapter with invalid number '{}'",
                c.number
            );
        }
        is_valid
    });

    if all_chapters.is_empty() {
        println!("No chapters found to clean.");
        return Ok(());
    }

    for path in &paths {
        fs::remove_file(path)
            .with_context(|| format!("Failed to delete original file: {}", path.display()))?;
    }

    println!("Deleted {} original file(s)\n", paths.len());

    write_clean_chapters(&translations_path, &all_chapters, yaml)?;

    println!("\nSuccessfully cleaned {} chapter(s).", all_chapters.len());

    Ok(())
}

pub fn clean_file(project: &str, file: usize, yaml: bool) -> Result<()> {
    let translations_path = &CONFIG.read().unwrap().get_translations_dir(project);
    let chapter_path = translations_path.join(format!("{}.md", file));

    if !chapter_path.exists() {
        bail!("File: `{}` not found.", &chapter_path.display());
    }

    backup(&chapter_path)?;

    let content = fs::read_to_string(&chapter_path)
        .with_context(|| format!("Failed to read file: {}", chapter_path.display()))?;
    let chapters = parse_chapters(&content);

    if chapters.is_empty() {
        println!("No chapters found in: {}", &chapter_path.display());
        return Ok(());
    }

    fs::remove_file(&chapter_path).with_context(|| {
        format!(
            "Failed to delete original file: {}",
            &chapter_path.display()
        )
    })?;

    let write_path = chapter_path.parent().unwrap_or(Path::new(""));

    write_clean_chapters(write_path, &chapters, yaml)?;

    Ok(())
}

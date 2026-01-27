use std::{fs, path::Path};

use anyhow::{Context, Result};
use rayon::iter::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator};

use super::Chapter;
use crate::clean::navigation::{yaml::*, *};

/// Writes a list of chapters to the disk with formatted names and navigation.
pub fn write_clean_chapters(write_path: &Path, chapters: &[Chapter], yaml: bool) -> Result<()> {
    let chapters_as_u32: Vec<_> = chapters.iter().filter_map(|c| c.number_as_u32()).collect();
    let max_digits = chapters_as_u32
        .iter()
        .max()
        .map_or(2, |n| n.to_string().len());

    chapters
        .par_iter()
        .enumerate()
        .try_for_each(|(i, chapter)| -> Result<()> {
            let chap_num = match chapter.number_as_u32() {
                Some(n) => n,
                None => return Ok(()),
            };

            let filename = format!("{:0>width$}.md", chap_num, width = max_digits);
            let filepath = write_path.join(filename);

            let (prev_chap, next_chap) =
                get_adjacent_chapter_numbers(write_path, &chapters_as_u32, i);

            let raw_content = format!(
                "# Chapter {}: {}\n\n{}\n\n",
                chapter.number, chapter.title, chapter.content
            );

            let final_content = if !yaml {
                wrap_with_nav_links(&raw_content, prev_chap, next_chap, max_digits)
            } else {
                yaml::add_front_matter(
                    &raw_content,
                    &chapter.title,
                    prev_chap,
                    next_chap,
                    max_digits,
                )
            };

            fs::write(&filepath, final_content.trim_end())
                .with_context(|| format!("Failed to write file: {}", filepath.display()))?;

            Ok(())
        })
}

pub fn sanitize_chapter(content: &str) -> String {
    let no_bom = content.trim_start_matches('\u{FEFF}');
    let no_front = remove_front_matter(no_bom);
    remove_nav_links(&no_front)
}

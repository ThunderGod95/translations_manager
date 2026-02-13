use anyhow::{Context, Result};
use rayon::iter::{IndexedParallelIterator, IntoParallelRefIterator, ParallelIterator};
use std::{fs, path::PathBuf};

use crate::{
    clean::{
        chapter::{Chapter, parse_chapters},
        formatter::{ChapterFormatter, RenderContext},
    },
    util::collect_numbered_file_paths,
};

#[derive(Debug)]
pub struct CleanOptions {
    pub translations_path: PathBuf,
    pub formatter: Box<dyn ChapterFormatter>,
    pub backup: bool,
    pub pad: bool,
}

impl CleanOptions {
    pub fn new(path: PathBuf, formatter: Box<dyn ChapterFormatter>) -> Self {
        Self {
            translations_path: path,
            formatter,
            backup: true,
            pad: true,
        }
    }
}

pub struct Pipeline;

impl Pipeline {
    pub fn execute(options: CleanOptions) -> Result<()> {
        println!("Scanning: {:?}", options.translations_path);

        let (paths, _) =
            collect_numbered_file_paths(&options.translations_path, Some("md"), None, None)?;

        println!("Found {} files.", paths.len());

        if options.backup {
            print!("Creating backup... ");

            use std::io::{self, Write};
            io::stdout().flush().ok();

            match crate::util::backup(&options.translations_path) {
                Ok(_) => println!("Done."),
                Err(e) => eprintln!("\nWarning: Backup failed: {}", e),
            }
        }

        println!("Parsing content...");

        let mut all_chapters = Vec::new();
        let mut files_to_delete = Vec::new();

        for path in &paths {
            let content = fs::read_to_string(path)
                .with_context(|| format!("Failed to read file: {}", path.display()))?;

            let parsed = parse_chapters(&content);

            if !parsed.is_empty() {
                all_chapters.extend(parsed);
                files_to_delete.push(path);
            } else {
                eprintln!("Warning: No chapters found in file: {}", path.display());
            }
        }

        let valid_chapters: Vec<&Chapter> = all_chapters
            .iter()
            .filter(|c| c.number_as_u32().is_some())
            .collect();

        if valid_chapters.is_empty() {
            eprintln!("No valid chapters found. Aborting.");
            return Ok(());
        }

        println!("Identified {} valid chapters.", valid_chapters.len());
        println!("Cleaning up old files...");

        for path in files_to_delete {
            if let Err(e) = fs::remove_file(path) {
                eprintln!(
                    "Warning: Failed to delete source file {}: {}",
                    path.display(),
                    e
                );
            }
        }

        let max_digits = if options.pad {
            valid_chapters
                .iter()
                .filter_map(|c| c.number_as_u32())
                .max()
                .map_or(2, |n| n.to_string().len())
        } else {
            0
        };

        println!("Formatting and writing new files...");

        valid_chapters
            .par_iter()
            .enumerate()
            .try_for_each(|(i, chapter)| -> Result<()> {
                let chap_num = chapter.number_as_u32().unwrap();

                let prev_id = if i > 0 {
                    valid_chapters[i - 1].number_as_u32()
                } else {
                    None
                };
                let next_id = if i < valid_chapters.len() - 1 {
                    valid_chapters[i + 1].number_as_u32()
                } else {
                    None
                };

                let ctx = RenderContext {
                    prev_id,
                    next_id,
                    width: max_digits,
                };

                let final_content = options.formatter.format(chapter, ctx);

                let filename = format!("{:0>width$}.md", chap_num, width = max_digits);
                let filepath = options.translations_path.join(filename);

                fs::write(&filepath, final_content)?;
                Ok(())
            })?;

        println!("Success! Processed {} chapters.", valid_chapters.len());

        Ok(())
    }
}

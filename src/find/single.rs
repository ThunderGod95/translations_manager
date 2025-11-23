use std::path::Path;

use anyhow::Result;
use fancy_regex::Regex;
use indicatif::{ParallelProgressIterator, ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::fs;

use crate::util::collect_numbered_file_paths;

/// Represents a single line match.
#[derive(Debug, Clone)]
pub struct Match {
    pub line_number: usize,
    pub line_content: String,
}

pub fn find_matches_in_folder(
    folder_path: impl AsRef<Path>,
    search_pattern: &str,
    use_regex: bool,
    start_file: Option<usize>,
    end_file: Option<usize>,
) -> Result<Vec<(usize, Vec<Match>)>> {
    let folder_path = folder_path.as_ref();

    let (files_to_read, _) =
        collect_numbered_file_paths(&folder_path, Some("md"), start_file, end_file)?;

    let pattern_string = search_pattern.to_string();

    let matcher = Matcher::new(&pattern_string, use_regex)?;

    let num_files = files_to_read.len() as u64;
    let pb = ProgressBar::new(num_files);

    pb.set_style(ProgressStyle::default_bar()
        .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({percent}%) - Searching files...")
        .expect("Failed to set progress bar style")
        .progress_chars("#>-")
    );

    let results: Vec<(usize, Vec<Match>)> = files_to_read
        .par_iter()
        .progress_with(pb)
        .filter_map(|path| {
            let file_number = path
                .file_prefix()
                .and_then(|s| s.to_str())
                .and_then(|name| name.parse::<usize>().ok())
                .unwrap_or(0); // We can safely unwrap because we know files_to_read only contains numbered files.

            let content = match fs::read_to_string(path) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("[WARN] Failed to read file {}: {}", path.display(), e);
                    return None;
                }
            };

            let matches = matcher.find_matches(&content);

            if matches.is_empty() {
                None
            } else {
                Some((file_number, matches))
            }
        })
        .collect();

    Ok(results)
}

pub fn find_matches_in_file(
    file_path: impl AsRef<Path>,
    search_pattern: &str,
    use_regex: bool,
) -> Result<Vec<Match>> {
    let content = fs::read_to_string(file_path)?;
    let matcher = Matcher::new(search_pattern, use_regex)?;
    Ok(matcher.find_matches(&content))
}

/// Internal struct to handle single-pattern matching.
struct Matcher {
    regex: Regex,
}

impl Matcher {
    fn new(needle: &str, use_regex: bool) -> Result<Self> {
        let needle = if use_regex {
            needle.to_owned()
        } else {
            super::shared::normalize_needle(needle)
        };

        Ok(Self {
            regex: Regex::new(&needle)?,
        })
    }

    fn find_matches(&self, haystack: &str) -> Vec<Match> {
        // If the regex doesn't match anywhere in the file, don't pay the cost
        // of splitting lines and allocating strings.
        if !self.regex.is_match(haystack).unwrap_or(false) {
            return Vec::new();
        }

        haystack
            .lines()
            .enumerate()
            .filter_map(|(num, line)| {
                let is_match = self.regex.is_match(line).unwrap_or(false);
                if is_match {
                    Some(Match {
                        line_number: num + 1,
                        line_content: line.to_string(),
                    })
                } else {
                    None
                }
            })
            .collect()
    }
}

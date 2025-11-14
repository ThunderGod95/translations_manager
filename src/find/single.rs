use std::io::{BufRead, BufReader};
use std::path::Path;

use anyhow::Result;
use rayon::prelude::*;
use regex::Regex;
use std::fs::File;

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

    let results: Vec<(usize, Vec<Match>)> = files_to_read
        .par_iter()
        .filter_map(|path| {
            let file_number = path
                .file_name()
                .and_then(|s| s.to_str())
                .and_then(|s| s.strip_suffix(".md"))
                .and_then(|name| name.parse::<usize>().ok())
                .unwrap_or(0); // We can safely unwrap because we know file_paths only contains numbered files.

            let file = match File::open(path) {
                Ok(f) => f,
                Err(e) => {
                    eprintln!("[WARN] Failed to open file {}: {}", path.display(), e);
                    return None;
                }
            };

            let reader = BufReader::new(file);

            let matches = matcher.find_matches(reader);

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
    let content = BufReader::new(File::open(file_path)?);
    let matcher = Matcher::new(search_pattern, use_regex)?;
    Ok(matcher.find_matches(content))
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

    fn find_matches(&self, haystack: BufReader<File>) -> Vec<Match> {
        let results = haystack
            .lines()
            .enumerate()
            .filter_map(|(num, line_result)| {
                line_result.ok().and_then(|line_content| {
                    if self.regex.is_match(&line_content) {
                        Some(Match {
                            line_number: num + 1,
                            line_content,
                        })
                    } else {
                        None
                    }
                })
            })
            .collect::<Vec<Match>>();

        results
    }
}

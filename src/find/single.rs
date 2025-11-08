use std::path::Path;

use anyhow::Result;
use rayon::prelude::*;
use regex::Regex;
use tokio::fs::read_to_string;
use tokio::task::spawn_blocking;

use crate::find::shared::read_files;

/// Represents a single line match.
#[derive(Debug, Clone)]
pub struct Match {
    pub line_number: usize,
    pub line_content: String,
}

pub async fn find_matches_in_folder(
    folder_path: impl AsRef<Path>,
    search_pattern: &str,
    use_regex: bool,
    start_file: Option<usize>,
    end_file: Option<usize>,
) -> Result<Vec<(usize, Vec<Match>)>> {
    let file_contents = read_files(&folder_path, start_file, end_file).await;

    let pattern_string = search_pattern.to_string();

    let matches_result = spawn_blocking(move || {
        let matcher = Matcher::new(&pattern_string, use_regex)?;

        let results = file_contents
            .par_iter()
            .map(|(index, content)| {
                matcher.find_matches(content).map(|matches| {
                    if matches.is_empty() {
                        None // No matches, becomes None
                    } else {
                        Some((*index, matches)) // Found matches
                    }
                })
            })
            .collect::<Result<Vec<_>, _>>();

        match results {
            Ok(list_with_nones) => {
                let final_list = list_with_nones
                    .into_iter()
                    .filter_map(|opt| opt)
                    .collect::<Vec<(usize, Vec<Match>)>>();
                Ok(final_list)
            }
            Err(e) => Err(e),
        }
    })
    .await;

    match matches_result {
        Ok(inner_result) => inner_result,
        Err(join_error) => Err(anyhow::Error::from(join_error)),
    }
}

pub async fn find_matches_in_file(
    file_path: impl AsRef<Path>,
    search_pattern: &str,
    use_regex: bool,
) -> Result<Vec<Match>> {
    let content = read_to_string(file_path).await?;
    let matcher = Matcher::new(search_pattern, use_regex)?;
    Ok(matcher.find_matches(&content)?)
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

    fn find_matches(&self, haystack: &str) -> Result<Vec<Match>> {
        let results = haystack
            .lines()
            .enumerate()
            .filter_map(|(num, line)| {
                if self.regex.is_match(line) {
                    Some(Match {
                        line_number: num + 1,
                        line_content: line.to_owned(),
                    })
                } else {
                    None
                }
            })
            .collect::<Vec<Match>>();

        Ok(results)
    }
}

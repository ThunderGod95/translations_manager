pub struct Match {
    pub line_number: usize,
    pub line_content: String,
}

fn normalize_needle(needle: &str) -> String {
    let starts_with_word = needle
        .chars()
        .next()
        .map_or(false, |c| c.is_alphanumeric() || c == '_');

    let ends_with_word = needle
        .chars()
        .last()
        .map_or(false, |c| c.is_alphanumeric() || c == '_');

    let normalized_search_pattern = regex::escape(needle);

    let prefix = if starts_with_word { "\\b" } else { "" };
    let suffix = if ends_with_word { "\\b" } else { "" };
    format!("(?i){}{}{}", prefix, normalized_search_pattern, suffix)
}

pub mod single {
    use std::collections::BTreeMap;
    use std::io;
    use std::{
        fs,
        path::{Path, PathBuf},
    };

    use anyhow::Result;
    use prettytable::{Cell, Row, Table, format};
    use rayon::prelude::*;
    use regex::Regex;

    use super::Match;
    use crate::util::get_file_contents_with_numbers;

    pub fn find_matches_in_folder(
        folder_path: impl AsRef<Path>,
        search_pattern: &str,
        use_regex: bool,
        start_file: Option<usize>,
        end_file: Option<usize>,
    ) -> Result<BTreeMap<PathBuf, Vec<Match>>> {
        let file_contents = get_file_contents_with_numbers(folder_path, start_file, end_file)?;
        let matcher = Matcher::new(search_pattern, use_regex)?;

        let results = file_contents
            .par_iter()
            .map(|(path, content)| {
                matcher.find_matches(content).map(|matches| {
                    if matches.is_empty() {
                        None
                    } else {
                        Some((path.clone(), matches))
                    }
                })
            })
            .collect::<Result<Vec<Option<(PathBuf, Vec<Match>)>>, _>>()?;

        let matches_by_files = results
            .into_iter()
            .filter_map(|opt| opt)
            .collect::<BTreeMap<PathBuf, Vec<Match>>>();

        Ok(matches_by_files)
    }

    #[derive(Debug, Clone, Copy)]
    pub enum FormatOption {
        Paragraphs,
        Table,
    }

    pub fn format_folder_matches<W: io::Write>(
        matches: &BTreeMap<PathBuf, Vec<Match>>,
        format_as: FormatOption,
        mut writer: W,
    ) -> Result<()> {
        match format_as {
            FormatOption::Table => write_table(matches, &mut writer),
            FormatOption::Paragraphs => write_paragraphs(matches, &mut writer),
        }
    }

    fn write_table<W: io::Write>(
        matches: &BTreeMap<PathBuf, Vec<Match>>,
        writer: &mut W,
    ) -> Result<()> {
        if matches.is_empty() {
            return Ok(());
        }

        let mut table = Table::new();

        table.set_format(*format::consts::FORMAT_NO_LINESEP);

        table.add_row(Row::from(vec!["File", "Line No.", "Text"]));

        for (path, matches_in_file) in matches {
            let file_name_str = path
                .file_stem()
                .unwrap_or(path.file_name().unwrap_or_default())
                .to_string_lossy();
            for m in matches_in_file {
                table.add_row(Row::new(vec![
                    Cell::new(&file_name_str),
                    Cell::new(&m.line_number.to_string()),
                    Cell::new(&m.line_content),
                ]));
            }
        }

        write!(writer, "{}", table)?;

        Ok(())
    }

    fn write_paragraphs<W: io::Write>(
        matches: &BTreeMap<PathBuf, Vec<Match>>,
        writer: &mut W,
    ) -> Result<()> {
        let mut matches_iter = matches.iter().peekable();

        while let Some((file, file_matches)) = matches_iter.next() {
            let file_name = file.file_name().unwrap_or_default();

            writeln!(writer, "# {}\n", file_name.display())?;

            for mat in file_matches {
                writeln!(writer, "[{}] {}\n", mat.line_number, mat.line_content)?;
            }

            if matches_iter.peek().is_some() {
                write!(writer, "\n---\n\n")?;
            }
        }

        Ok(())
    }

    pub fn find_matches_in_file(
        file_path: impl AsRef<Path>,
        search_pattern: &str,
        use_regex: bool,
    ) -> Result<Vec<Match>> {
        let content = fs::read_to_string(file_path)?;

        let matcher = Matcher::new(search_pattern, use_regex)?;

        Ok(matcher.find_matches(&content)?)
    }

    pub struct Matcher {
        regex: Regex,
    }

    impl Matcher {
        pub fn new(needle: &str, use_regex: bool) -> Result<Self> {
            let needle = if use_regex {
                needle.to_owned()
            } else {
                super::normalize_needle(needle)
            };

            Ok(Self {
                regex: Regex::new(&needle)?,
            })
        }

        pub fn find_matches(&self, haystack: &str) -> Result<Vec<Match>> {
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
}

pub mod multi {
    use std::{
        collections::HashMap,
        fs,
        path::{Path, PathBuf},
    };

    use anyhow::Result;
    use regex::RegexSet;

    use crate::util::get_file_contents_with_numbers;

    #[derive(Debug, Clone)]
    pub struct MultiMatch {
        pub line_number: usize,
        pub line_content: String,
        pub matched_patterns: Vec<String>,
    }

    pub fn find_matches_in_folder(
        folder_path: impl AsRef<Path>,
        search_patterns: &[&str],
        use_regex: bool,
        start_file: Option<usize>,
        end_file: Option<usize>,
    ) -> Result<HashMap<PathBuf, Vec<MultiMatch>>> {
        let file_contents = get_file_contents_with_numbers(folder_path, start_file, end_file)?;

        let mut matches_by_files = HashMap::with_capacity(file_contents.len());

        let matcher = MultiMatcher::new(search_patterns, use_regex)?;

        for (path, content) in file_contents {
            let matches = matcher.find_matches(&content)?;

            if !matches.is_empty() {
                matches_by_files.insert(path, matches);
            }
        }

        Ok(matches_by_files)
    }

    pub fn find_matches_in_file(
        file_path: impl AsRef<Path>,
        search_patterns: &[&str],
        use_regex: bool,
    ) -> Result<Vec<MultiMatch>> {
        let content = fs::read_to_string(file_path)?;

        let matcher = MultiMatcher::new(search_patterns, use_regex)?;

        Ok(matcher.find_matches(&content)?)
    }

    pub struct MultiMatcher {
        regex_set: RegexSet,
        original_patterns: Vec<String>,
    }

    impl MultiMatcher {
        pub fn new(patterns: &[&str], use_regex: bool) -> Result<Self> {
            let original_patterns: Vec<String> = patterns.iter().map(|s| s.to_string()).collect();

            let processed_patterns: Vec<String> = if use_regex {
                original_patterns.clone()
            } else {
                original_patterns
                    .iter()
                    .map(|s| super::normalize_needle(s))
                    .collect()
            };

            Ok(Self {
                regex_set: RegexSet::new(processed_patterns)?,
                original_patterns,
            })
        }

        pub fn find_matches(&self, haystack: &str) -> Result<Vec<MultiMatch>> {
            let mut results = vec![];

            for (num, line) in haystack.lines().enumerate() {
                let matches: Vec<usize> = self.regex_set.matches(line).into_iter().collect();

                if !matches.is_empty() {
                    let matched_patterns = matches
                        .into_iter()
                        .map(|index| self.original_patterns[index].clone())
                        .collect();

                    results.push(MultiMatch {
                        line_number: num + 1,
                        line_content: line.to_owned(),
                        matched_patterns,
                    });
                }
            }

            Ok(results)
        }
    }
}

use anyhow::{Context, Result, bail};
use fancy_regex::{Captures, Regex};
use jwalk::WalkDir;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use std::{fs, path::Path, sync::Arc};

pub fn replace(search_regex: &Regex, replacement: &str, haystack: &str) -> (usize, String) {
    let mut count = 0;

    let new_haystack = search_regex.replace_all(haystack, |_caps: &Captures| {
        count += 1;
        replacement
    });

    (count, new_haystack.into_owned())
}

fn process_file(
    file_path: &Path,
    search_regex: Arc<Regex>,
    replacement: Arc<String>,
) -> Result<usize> {
    let content = fs::read_to_string(&file_path)
        .with_context(|| format!("Failed to read file: {}", file_path.display()))?;

    let (count, new_content) = replace(&search_regex, &replacement, &content);

    if count > 0 {
        fs::write(&file_path, new_content)
            .with_context(|| format!("Failed to write changes to file: {}", file_path.display()))?;

        println!(
            "Updated {} ({} replacements)",
            file_path.file_name().unwrap().display(),
            count
        );
    }

    Ok(count)
}

pub fn replace_in_folder(
    folder_path: impl AsRef<Path>,
    search_pattern: &str,
    replacement: &str,
    use_regex: bool,
) -> Result<(usize, usize)> {
    let folder_path = folder_path.as_ref();

    if !folder_path.exists() {
        bail!(
            "The folder '{}' does not exist. Please check the path.",
            folder_path.display()
        );
    }

    let search_regex = if use_regex {
        Regex::new(search_pattern)
            .with_context(|| format!("Invalid regex pattern provided: '{}'", search_pattern))?
    } else {
        Regex::new(&format!(r"(?i)\b{}\b", fancy_regex::escape(search_pattern)))
            .with_context(|| format!("Invalid search pattern provided: '{}'", search_pattern))?
    };

    let files: Vec<_> = WalkDir::new(folder_path)
        .max_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .map(|e| e.path())
        .collect();

    let search_regex = Arc::new(search_regex);
    let replacement = Arc::new(replacement.to_string());

    let results: Vec<Result<usize>> = files
        .par_iter()
        .map(|path| process_file(path, Arc::clone(&search_regex), Arc::clone(&replacement)))
        .collect();

    let mut total_replacements = 0;
    let mut total_files_updated = 0;

    for result in results {
        match result {
            Ok(count) => {
                if count > 0 {
                    total_replacements += count;
                    total_files_updated += 1;
                }
            }
            Err(e) => {
                eprintln!("Failed to process a file: {}", e);
            }
        }
    }

    Ok((total_replacements, total_files_updated))
}

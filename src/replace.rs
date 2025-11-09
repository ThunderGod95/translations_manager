use anyhow::{Context, Result};
use futures::future::join_all;
use log::{error, info};
use regex::{Captures, Regex};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::fs;

pub fn replace(search_regex: &Regex, replacement: &str, haystack: &str) -> (String, usize) {
    let mut count = 0;

    let new_haystack = search_regex.replace_all(haystack, |_caps: &Captures| {
        count += 1;
        replacement
    });

    (new_haystack.into_owned(), count)
}

async fn process_file(
    file_path: PathBuf,
    search_regex: Arc<Regex>,
    replacement: Arc<String>,
) -> Result<(PathBuf, usize)> {
    let content = match fs::read_to_string(&file_path).await {
        Ok(content) => content,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            info!("Skipping file {}: Not found.", file_path.display());
            return Ok((file_path, 0));
        }
        Err(e) => {
            return Err(e).with_context(|| format!("Failed to read file: {}", file_path.display()));
        }
    };

    let (new_content, count) = replace(&search_regex, &replacement, &content);

    if count > 0 {
        fs::write(&file_path, new_content)
            .await
            .with_context(|| format!("Failed to write changes to file: {}", file_path.display()))?;

        info!(
            "Updated {} ({} replacements)",
            file_path.file_name().unwrap().display(),
            count
        );
    }

    Ok((file_path, count))
}

pub async fn replace_in_folder(
    folder_path: impl AsRef<Path>,
    search_pattern: &str,
    replacement: &str,
    use_regex: bool,
) -> Result<(usize, usize)> {
    let search_regex = if use_regex {
        Regex::new(search_pattern)
            .with_context(|| format!("Invalid regex pattern provided: '{}'", search_pattern))?
    } else {
        Regex::new(&format!(r"(?i)\b{}\b", regex::escape(search_pattern)))
            .with_context(|| format!("Invalid search pattern provided: '{}'", search_pattern))?
    };

    let search_regex = Arc::new(search_regex);
    let replacement = Arc::new(replacement.to_string());

    let mut tasks = Vec::new();

    let mut dir = fs::read_dir(&folder_path).await.with_context(|| {
        format!(
            "Failed to read directory: {}",
            folder_path.as_ref().display()
        )
    })?;

    while let Some(entry) = dir.next_entry().await.with_context(|| {
        format!(
            "Failed to read entry in directory: {}",
            folder_path.as_ref().display()
        )
    })? {
        let path = entry.path();

        let file_type = match entry.file_type().await {
            Ok(ft) => ft,
            Err(e) => {
                error!(
                    "Could not determine file type for {}: {}. Skipping.",
                    path.display(),
                    e
                );
                continue;
            }
        };

        if file_type.is_file() {
            tasks.push(tokio::spawn(process_file(
                path,
                Arc::clone(&search_regex),
                Arc::clone(&replacement),
            )));
        }
    }

    let results = join_all(tasks).await;
    let mut total_replacements = 0;
    let mut total_files_updated = 0;

    for result in results {
        match result {
            Err(join_err) => {
                error!(
                    "A file processing task failed unexpectedly (panicked): {}",
                    join_err
                );
            }
            Ok(Err(proc_err)) => {
                error!("Failed to process file: {:#}", proc_err);
            }
            Ok(Ok((_path, count))) => {
                total_replacements += count;
                total_files_updated += 1;
            }
        }
    }

    Ok((total_replacements, total_files_updated))
}

use std::{
    path::{Path, PathBuf},
    sync::Arc,
    usize,
};

use futures::future::join_all;
use tokio::{
    fs::{read_dir, read_to_string},
    sync::Semaphore,
};

/// Normalizes a plain string into a case-insensitive regex
/// that respects word boundaries.
pub(super) fn normalize_needle(needle: &str) -> String {
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

pub async fn get_filtered_file_paths(
    folder_path: impl AsRef<Path>,
    start_num: Option<usize>,
    end_num: Option<usize>,
) -> Vec<(usize, PathBuf)> {
    let Ok(mut read_dir) = read_dir(folder_path).await else {
        return vec![];
    };

    let mut all_file_paths = vec![];

    while let Ok(Some(entry)) = read_dir.next_entry().await {
        let file_name_os = entry.file_name();

        let id = file_name_os
            .to_str()
            .and_then(|s| s.strip_suffix(".md"))
            .and_then(|name| name.parse::<usize>().ok());

        if let Some(id) = id {
            if let Ok(file_type) = entry.file_type().await {
                if file_type.is_file() {
                    all_file_paths.push((id, entry.path()));
                }
            }
        }
    }

    all_file_paths.sort_unstable_by_key(|(id, _)| *id);

    let start = start_num.unwrap_or(usize::MIN);
    let end = end_num.unwrap_or(usize::MAX);

    let filtered_paths = all_file_paths
        .into_iter()
        .filter(|(id, _)| *id >= start && *id <= end)
        .collect::<Vec<_>>();

    filtered_paths
}

pub async fn read_files(
    folder_path: impl AsRef<Path>,
    start_num: Option<usize>,
    end_num: Option<usize>,
) -> Vec<(usize, String)> {
    let files_to_read = get_filtered_file_paths(&folder_path, start_num, end_num).await;

    if files_to_read.is_empty() {
        return vec![];
    }

    let semaphore = Arc::new(Semaphore::new(200));
    let mut futures = Vec::with_capacity(files_to_read.len());

    for (id, path) in files_to_read {
        let sem_clone = Arc::clone(&semaphore);

        let fut = async move {
            let _permit = sem_clone.acquire_owned().await.unwrap();

            let content_result = read_to_string(&path).await;

            (id, content_result)
        };

        futures.push(fut);
    }

    let results = join_all(futures).await;

    let mut final_vec = results
        .into_iter()
        .filter_map(|(id, content_result)| content_result.ok().map(|content| (id, content)))
        .collect::<Vec<_>>();

    final_vec.sort_unstable_by_key(|(i, _)| *i);

    final_vec
}

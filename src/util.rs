use std::{
    collections::HashMap,
    ffi::{OsStr, OsString},
    fs::{self, read_to_string},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow, bail};
use console::Style;
use directories::ProjectDirs;
use once_cell::sync::Lazy;
use rayon::prelude::*;
use tokio::process::Command;

use crate::config::{CONFIG, PROJECT_PATH_QUALIFIERS};

static RED: Lazy<Style> = Lazy::new(|| Style::new().red());
static GREEN: Lazy<Style> = Lazy::new(|| Style::new().green());
static YELLOW: Lazy<Style> = Lazy::new(|| Style::new().yellow());
static _CYAN: Lazy<Style> = Lazy::new(|| Style::new().cyan());

pub fn log_error<T: std::fmt::Display>(e: T) {
    eprintln!("{}", RED.apply_to(e));
}

pub fn log_warning<T: std::fmt::Display>(e: T) {
    eprintln!("{}", YELLOW.apply_to(e));
}

pub fn log_info<T: std::fmt::Display>(e: T) {
    println!("{}", e);
}

pub fn log_success<T: std::fmt::Display>(e: T) {
    println!("{}", GREEN.apply_to(e));
}

pub fn get_config_file_path() -> Result<PathBuf> {
    let project_dirs = ProjectDirs::from(
        PROJECT_PATH_QUALIFIERS[0],
        PROJECT_PATH_QUALIFIERS[1],
        PROJECT_PATH_QUALIFIERS[2],
    )
    .ok_or_else(|| anyhow::anyhow!("Could not determine project directories"))?;

    let config_dir = project_dirs.config_dir();
    fs::create_dir_all(config_dir)?;
    Ok(config_dir.join("config.toml"))
}

pub fn get_cache_path() -> Result<PathBuf> {
    let project_dirs = ProjectDirs::from(
        PROJECT_PATH_QUALIFIERS[0],
        PROJECT_PATH_QUALIFIERS[1],
        PROJECT_PATH_QUALIFIERS[2],
    )
    .ok_or_else(|| anyhow::anyhow!("Could not determine project directories"))?;

    let path = project_dirs.cache_dir();
    fs::create_dir_all(path)?;

    // Use the filename from the loaded config
    Ok(path.join(&CONFIG.cache_file))
}

pub fn get_find_history_config_path() -> Result<PathBuf> {
    let project_dirs = ProjectDirs::from(
        PROJECT_PATH_QUALIFIERS[0],
        PROJECT_PATH_QUALIFIERS[1],
        PROJECT_PATH_QUALIFIERS[2],
    )
    .ok_or_else(|| anyhow::anyhow!("Could not determine project directories"))?;

    let path = project_dirs.config_dir();
    fs::create_dir_all(path)?;

    // Use the filename from the loaded config
    Ok(path.join(&CONFIG.find_history_config_file))
}

pub fn get_sorted_md_file_paths(folder_path: impl AsRef<Path>) -> Result<Vec<PathBuf>> {
    let folder_path = folder_path.as_ref();

    if !folder_path.is_dir() {
        return Err(anyhow!(
            "The path '{}' does not exist or is a not a directory.",
            folder_path.display()
        ));
    }

    let files = folder_path.read_dir()?;

    let mut paths: Vec<_> = files
        .filter_map(|entry_result| {
            let entry = entry_result.ok()?;
            let file_type = entry.file_type().ok()?;

            if !file_type.is_file() {
                return None;
            }

            let path = entry.path();

            if path.extension().map_or(false, |ext| ext == "md") {
                Some(path)
            } else {
                None
            }
        })
        .collect();

    paths.sort_by_key(|path| {
        path.file_stem()
            .and_then(|stem| stem.to_str())
            .and_then(|stem_str| stem_str.parse::<usize>().ok())
            .unwrap_or(usize::MAX)
    });

    Ok(paths)
}

pub fn get_file_contents_with_numbers(
    folder_path: impl AsRef<Path>,
    start_num: Option<usize>,
    end_num: Option<usize>,
) -> Result<HashMap<PathBuf, String>> {
    let file_paths = get_sorted_md_file_paths(folder_path)?;

    file_paths
        .into_par_iter()
        .filter_map(|path| {
            let file_num = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .and_then(|stem_str| stem_str.parse::<usize>().ok());

            if let Some(num) = file_num {
                let in_start_range = start_num.map_or(true, |s| num >= s);
                let in_end_range = end_num.map_or(true, |s| num <= s);

                if in_start_range && in_end_range {
                    Some(
                        read_to_string(&path)
                            .with_context(|| format!("Failed to read file: {}", path.display()))
                            .map(|content| (path, content)),
                    )
                } else {
                    None
                }
            } else {
                None
            }
        })
        .collect()
}

pub fn normalize_path(path: impl AsRef<str>) -> String {
    path.as_ref().replace("\\", "/")
}

pub async fn open_in_vs_code(file_paths: &[impl AsRef<OsStr>]) {
    if file_paths.is_empty() {
        log_warning("No file paths provided to open in VS Code.".to_string());
        return;
    }

    let paths_owned: Vec<OsString> = file_paths.iter().map(|p| p.as_ref().to_owned()).collect();

    let mut cmd;

    if cfg!(target_os = "windows") {
        cmd = Command::new("cmd");
        cmd.arg("/C");
        cmd.arg("code");
    } else {
        cmd = Command::new("sh");
        cmd.arg("-c");
        cmd.arg("code \"$@\"");
        cmd.arg("_");
    };

    cmd.args(&paths_owned);

    log_info(format!(
        "✅ Opening {} file/folder(s) in VS Code...",
        paths_owned.len(),
    ));

    match cmd.output().await {
        Ok(output) => {
            if output.status.success() {
                log_info(format!(
                    "✅ Opended {} file/folder(s) in VS Code successfully.",
                    paths_owned.len()
                ));
            } else {
                log_error(format!(
                    "⚠️ VS Code task finished with a non-success status: {}",
                    output.status
                ));

                let stderr = String::from_utf8_lossy(&output.stderr);
                if !stderr.trim().is_empty() {
                    log_error(format!("VS Code stderr:\n{}", stderr.trim()));
                }

                let stdout = String::from_utf8_lossy(&output.stdout);
                if !stdout.trim().is_empty() {
                    log_info(format!("VS Code stdout:\n{}", stdout.trim()));
                }
            }
        }
        Err(e) => {
            log_warning(format!(
                "⚠️ Could not execute VS Code task. Is 'code' in your system's PATH? Error: {}",
                e
            ));
        }
    }
}

pub fn sort_indexed_results<T>(results: Vec<Result<(usize, T)>>) -> Result<Vec<T>> {
    let mut ok_results = Vec::with_capacity(results.len());

    for result in results {
        let indexed_payload = result.with_context(
            || "File read/process error: One of the files failed to be read or processed.",
        )?;

        ok_results.push(indexed_payload);
    }

    ok_results.sort_by_key(|(i, _)| *i);

    for window in ok_results.windows(2) {
        let (current_i, _) = window[0];
        let (next_i, _) = window[1];

        if next_i != current_i + 1 {
            bail!(
                "File sequence error: Found file {}.md, but file {}.md is missing.",
                current_i,
                next_i
            );
        }
    }

    let sorted_payloads = ok_results.into_iter().map(|(_, payload)| payload).collect();

    Ok(sorted_payloads)
}

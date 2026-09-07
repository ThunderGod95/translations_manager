use std::{
    collections::HashSet,
    ffi::OsStr,
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow, bail};
use directories::ProjectDirs;
use time::{OffsetDateTime, macros::format_description};

use crate::cli::config::{CONFIG, PROJECT_PATH_QUALIFIERS};

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
    Ok(path.join(&CONFIG.read().unwrap().cache_file))
}

pub fn normalize_path(path: impl AsRef<str>) -> String {
    path.as_ref().replace("\\", "/")
}

/// Checks if the app is the only process attached to the console.
pub fn is_standalone() -> bool {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Console::GetConsoleProcessList;
        let mut process_list: [u32; 2] = [0; 2];
        let process_count = unsafe { GetConsoleProcessList(process_list.as_mut_ptr(), 2) };

        // If count is 1, we are standalone (e.g., double-clicked).
        // If 0, no console (e.g., in background).
        // If 2+, running from an existing terminal (e.g., cmd, powershell).
        process_count == 1
    }
    #[cfg(not(windows))]
    {
        // This feature is Windows-specific, so default to "not standalone"
        // on other platforms, meaning it will always run once and exit.
        false
    }
}

/// Waits for user input to either re-run or quit.
/// Returns `true` to re-run, `false` to quit.
pub fn prompt_for_rerun() -> bool {
    eprintln!("\nPress 'Enter' to quit, or any other key to run again...");

    let term = console::Term::stdout();

    term.flush().unwrap();

    let key_result = term.read_key();

    match key_result {
        Ok(console::Key::Enter) => false,
        Ok(_) => true,
        Err(e) => {
            eprintln!("Failed to read key: {}", e);
            false
        }
    }
}

/// Checks if a file at the given path is "empty".
///
/// An "empty" file is one that either:
/// 1. Has a size of 0 bytes.
/// 2. Contains *only* characters that are NOT alphanumeric or ASCII punctuation/symbols.
///    (e.g., it contains only whitespace, control, or format characters).
/// 3. Does not exist or we do not have permission reading.
pub fn is_file_empty(path: impl AsRef<Path>) -> bool {
    let path = path.as_ref();

    match fs::metadata(path) {
        Ok(metadata) => {
            if metadata.len() == 0 {
                true
            } else {
                let content = match fs::read_to_string(path) {
                    Ok(c) => c,
                    Err(_) => return true,
                };

                is_string_empty(&content)
            }
        }
        Err(_) => true,
    }
}

pub fn is_string_empty(txt: &str) -> bool {
    !txt.chars()
        .any(|c| c.is_alphanumeric() || c.is_ascii_punctuation())
}

pub fn get_current_date() -> Result<String> {
    let format = format_description!("[year]-[month]-[day]");
    let now_utc = OffsetDateTime::now_utc();
    now_utc
        .date()
        .format(&format)
        .context("Failed to get current date.")
}

pub fn get_current_date_time() -> Result<String> {
    let format = format_description!("[year]_[month]_[day]-[hour]_[minute]");
    let now_utc = OffsetDateTime::now_utc();
    now_utc
        .format(&format)
        .context("Failed to format current date and time.")
}

pub fn collect_numbered_file_paths(
    input_dir: &Path,
    format: Option<&str>,
    start: Option<usize>,
    end: Option<usize>,
) -> Result<(Vec<PathBuf>, u64)> {
    let check_format = format.is_some();
    let format = format.unwrap_or("");

    let mut entries: Vec<(usize, PathBuf, u64)> = std::fs::read_dir(input_dir)
        .with_context(|| format!("Failed to read directory: {}", input_dir.display()))?
        .filter_map(Result::ok)
        .filter_map(|entry| {
            if !entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) {
                return None;
            }

            let path = entry.path();

            if check_format {
                if path.extension() != Some(OsStr::new(format)) {
                    return None;
                }
            }

            let file_num = path
                .file_stem()
                .and_then(OsStr::to_str)
                .and_then(|s| s.parse::<usize>().ok())?;

            if start.map_or(false, |s| file_num < s) || end.map_or(false, |e| file_num > e) {
                return None;
            }

            match entry.metadata() {
                Ok(md) => Some(Ok((file_num, path.to_path_buf(), md.len()))),
                Err(e) => Some(Err(anyhow!(
                    "Failed to get metadata for {}: {}",
                    path.display(),
                    e
                ))),
            }
        })
        .collect::<Result<Vec<_>>>()
        .context("Failed while scanning directory for files")?;

    entries.sort_by_key(|(file_num, _, _)| *file_num);

    if let (Some(s), Some(e)) = (start, end) {
        if s > e {
            bail!("Validation failed: Start value ({s}) cannot be greater than end value ({e}).");
        }

        let expected_count = e.saturating_sub(s).saturating_add(1);

        if entries.len() != expected_count {
            let found_nums: HashSet<_> = entries.iter().map(|(num, _, _)| *num).collect();
            let missing_files: Vec<_> = (s..=e).filter(|n| !found_nums.contains(n)).collect();

            bail!(
                "Validation failed: Expected {expected_count} files in range {s}-{e}, but only found {}.\n\tMissing file numbers: {:?}",
                entries.len(),
                missing_files
            );
        }
    }

    let mut file_paths = Vec::with_capacity(entries.len());
    let mut total_size = 0u64;

    for (_, path, size) in entries {
        file_paths.push(path);
        total_size = total_size
            .checked_add(size)
            .ok_or_else(|| anyhow!("Total size overflow while summing file sizes"))?;
    }

    Ok((file_paths, total_size))
}

pub fn backup(og_path: &Path) -> Result<()> {
    if og_path.is_dir() {
        backup::backup_folder(&og_path)
    } else {
        backup::backup_file(&og_path)
    }
}

mod backup {
    use std::path::Path;
    use std::{fs, path::PathBuf};

    use anyhow::{Context, Result, anyhow};
    use rayon::iter::{ParallelBridge, ParallelIterator};

    use super::get_current_date_time;

    pub(super) fn backup_folder(source_path: &Path) -> Result<()> {
        let backup_root = resolve_backup_root(source_path)?;

        create_dir_all_verbose(&backup_root)?;
        copy_recursive(source_path, &backup_root)?;

        Ok(())
    }

    pub(super) fn backup_file(file_path: &Path) -> Result<()> {
        let parent = file_path.parent().unwrap_or_else(|| Path::new("."));
        let backup_root = resolve_backup_root(parent)?;

        create_dir_all_verbose(&backup_root)?;
        copy_file(file_path, &backup_root)?;

        Ok(())
    }

    fn copy_recursive(source: &Path, dest: &Path) -> Result<()> {
        fs::read_dir(source)?
            .par_bridge()
            .try_for_each(|entry| -> Result<()> {
                let entry = entry?;
                let file_name = entry.file_name();

                if file_name == ".backup" {
                    return Ok(());
                }

                let source_path = entry.path();
                let dest_path = dest.join(&file_name);
                let file_type = entry.file_type()?;

                if file_type.is_dir() {
                    if let Err(e) = fs::create_dir(&dest_path) {
                        if e.kind() != std::io::ErrorKind::AlreadyExists {
                            return Err(e).context(format!(
                                "Failed to create subdir: {}",
                                dest_path.display()
                            ));
                        }
                    }
                    copy_recursive(&source_path, &dest_path)?;
                } else {
                    copy_file_explicit(&source_path, &dest_path)?;
                }

                Ok(())
            })
    }

    fn resolve_backup_root(base: &Path) -> Result<PathBuf> {
        Ok(base.join(".backup").join(get_current_date_time()?))
    }

    fn create_dir_all_verbose(path: &Path) -> Result<()> {
        fs::create_dir_all(path)
            .with_context(|| format!("Failed to create backup root: {}", path.display()))
    }

    fn copy_file(source: &Path, dest_root: &Path) -> Result<()> {
        let file_name = source
            .file_name()
            .ok_or_else(|| anyhow!("Invalid file name: {}", source.display()))?;

        let dest_path = dest_root.join(file_name);
        copy_file_explicit(source, &dest_path)
    }

    /// Performs the actual copy to a specific full path
    fn copy_file_explicit(source: &Path, dest: &Path) -> Result<()> {
        fs::copy(source, dest).with_context(|| {
            format!("Failed to copy {} to {}", source.display(), dest.display())
        })?;
        Ok(())
    }
}

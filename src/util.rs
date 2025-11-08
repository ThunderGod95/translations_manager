use std::{
    ffi::{OsStr, OsString},
    fs::{self},
    path::PathBuf,
};

use anyhow::Result;
use directories::ProjectDirs;
use log::{error, info, warn};
use tokio::process::Command;

use crate::config::{CONFIG, PROJECT_PATH_QUALIFIERS};

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

    let path = project_dirs.cache_dir();
    fs::create_dir_all(path)?;

    // Use the filename from the loaded config
    Ok(path.join(&CONFIG.find_history_config_file))
}

pub fn normalize_path(path: impl AsRef<str>) -> String {
    path.as_ref().replace("\\", "/")
}

pub async fn open_in_vs_code(file_paths: &[impl AsRef<OsStr>]) {
    if file_paths.is_empty() {
        error!("No file paths provided to open in VS Code.");
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

    info!(
        "✅ Opening {} file/folder(s) in VS Code...",
        paths_owned.len(),
    );

    match cmd.output().await {
        Ok(output) => {
            if !output.status.success() {
                warn!(
                    "⚠️ VS Code task finished with a non-success status: {}",
                    output.status
                );

                let stderr = String::from_utf8_lossy(&output.stderr);
                if !stderr.trim().is_empty() {
                    error!("VS Code stderr:\n{}", stderr.trim());
                }

                let stdout = String::from_utf8_lossy(&output.stdout);
                if !stdout.trim().is_empty() {
                    warn!("VS Code stdout:\n{}", stdout.trim());
                }
            }
        }
        Err(e) => {
            warn!(
                "⚠️ Could not execute VS Code task. Is 'code' in your system's PATH? Error: {}",
                e
            );
        }
    }
}

/// Pauses for user input if the app is the only process
/// attached to the console (i.e., was not run from an
/// existing terminal).
pub async fn wait_for_input_if_standalone() {
    #[cfg(windows)]
    {
        use windows_sys::Win32::System::Console::GetConsoleProcessList;
        let mut process_list: [u32; 2] = [0; 2];

        let process_count = unsafe { GetConsoleProcessList(process_list.as_mut_ptr(), 2) };

        if process_count == 1 {
            use tokio::io::{AsyncBufReadExt, BufReader};

            eprintln!("\nPress any key to close the window...");

            let mut stdin = BufReader::new(tokio::io::stdin());
            let mut _buffer = String::new();

            let _ = stdin.read_line(&mut _buffer).await;
        }
    }

    // Silence warnings on non-windows builds
    #[cfg(not(windows))]
    {
        // No-op
        let _ = tokio::time::sleep(std::time::Duration::from_millis(0));
    }
}

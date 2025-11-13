use std::{
    ffi::{OsStr, OsString},
    path::PathBuf,
};

use anyhow::Result;
use directories::ProjectDirs;
use log::warn;
use tokio::{fs::create_dir_all, process::Command};

use crate::config::{PROJECT_PATH_QUALIFIERS, get_config};

pub async fn get_config_file_path() -> Result<PathBuf> {
    let project_dirs = ProjectDirs::from(
        PROJECT_PATH_QUALIFIERS[0],
        PROJECT_PATH_QUALIFIERS[1],
        PROJECT_PATH_QUALIFIERS[2],
    )
    .ok_or_else(|| anyhow::anyhow!("Could not determine project directories"))?;

    let config_dir = project_dirs.config_dir();
    create_dir_all(config_dir).await?;
    Ok(config_dir.join("config.toml"))
}

pub async fn get_cache_path() -> Result<PathBuf> {
    let project_dirs = ProjectDirs::from(
        PROJECT_PATH_QUALIFIERS[0],
        PROJECT_PATH_QUALIFIERS[1],
        PROJECT_PATH_QUALIFIERS[2],
    )
    .ok_or_else(|| anyhow::anyhow!("Could not determine project directories"))?;

    let path = project_dirs.cache_dir();
    create_dir_all(path).await?;

    // Use the filename from the loaded config
    Ok(path.join(&get_config().await.cache_file))
}

pub async fn get_find_history_config_path() -> Result<PathBuf> {
    let project_dirs = ProjectDirs::from(
        PROJECT_PATH_QUALIFIERS[0],
        PROJECT_PATH_QUALIFIERS[1],
        PROJECT_PATH_QUALIFIERS[2],
    )
    .ok_or_else(|| anyhow::anyhow!("Could not determine project directories"))?;

    let path = project_dirs.cache_dir();
    create_dir_all(path).await?;

    Ok(path.join(&get_config().await.find_history_config_file))
}

pub fn normalize_path(path: impl AsRef<str>) -> String {
    path.as_ref().replace("\\", "/")
}

pub async fn open_in_vs_code(file_paths: &[impl AsRef<OsStr>]) {
    if file_paths.is_empty() {
        eprintln!("No file paths provided to open in VS Code.");
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

    println!("Opening {} file/folder(s) in VS Code...", paths_owned.len(),);

    match cmd.output().await {
        Ok(output) => {
            if !output.status.success() {
                warn!(
                    "⚠️ VS Code task finished with a non-success status: {}",
                    output.status
                );

                let stderr = String::from_utf8_lossy(&output.stderr);
                if !stderr.trim().is_empty() {
                    eprintln!("VS Code stderr:\n{}", stderr.trim());
                }

                let stdout = String::from_utf8_lossy(&output.stdout);
                if !stdout.trim().is_empty() {
                    eprintln!("VS Code stdout:\n{}", stdout.trim());
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
pub async fn prompt_for_rerun() -> bool {
    eprintln!("\nPress 'Enter' to quit, or any other key to run again...");

    let key_result = tokio::task::spawn_blocking(|| {
        let term = console::Term::stdout();
        term.read_key()
    })
    .await;

    match key_result {
        Ok(Ok(console::Key::Enter)) => false,
        Ok(_) => true,
        Err(e) => {
            eprintln!("Failed to read key: {}", e);
            false
        }
    }
}

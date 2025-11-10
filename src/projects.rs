use std::path::Path;

use anyhow::{Context, Result, bail};
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Confirm, Select};
use log::{info, warn};
use tokio::fs;

use crate::cache::*;

pub async fn select_project(base_path: impl AsRef<Path>) -> Result<String> {
    let base_path = base_path.as_ref();
    let projects = get_projects(base_path).await?;

    if projects.len() == 1 {
        let project_name = projects.first().unwrap().clone();
        info!(
            "\n✅ Only one project found. Auto-selecting: {}",
            project_name
        );

        if let Err(e) = write_cache(Cache {
            last_project: project_name.clone(),
        })
        .await
        {
            warn!("Could not write to cache file: {}", e);
        }

        return Ok(project_name);
    }

    if let Some(cache) = read_cache().await? {
        let last_project = cache.last_project;

        if projects.binary_search(&last_project).is_ok() {
            let use_last = Confirm::with_theme(&ColorfulTheme::default())
                .with_prompt(format!("Use last selected project: {}", &last_project))
                .default(true)
                .show_default(true)
                .interact()
                .context("Failed to render confirmation prompt")?;

            if use_last {
                info!("\n✅ Using cached project: {}\n", &last_project);
                return Ok(last_project);
            }
        }
    }

    let project_index = Select::with_theme(&ColorfulTheme::default())
        .with_prompt("Select a translation project:")
        .items(&projects)
        .default(0)
        .interact()
        .context("Failed to render selection prompt")?;

    let selected_project = projects[project_index].clone();

    if let Err(e) = write_cache(Cache {
        last_project: selected_project.clone(),
    })
    .await
    {
        warn!("Warning: Could not write to cache file: {}", e);
    }

    Ok(selected_project)
}

pub async fn get_projects(base_path: &Path) -> Result<Vec<String>> {
    let mut projects: Vec<String> = Vec::new();

    let mut read_dir = fs::read_dir(base_path)
        .await
        .with_context(|| format!("Failed to read projects from {}", base_path.display()))?;

    while let Some(entry) = read_dir.next_entry().await? {
        let file_type = match entry.file_type().await {
            Ok(ft) => ft,
            Err(_) => continue,
        };

        let file_name = entry.file_name();

        if !file_type.is_dir() || file_name == "tscripts" {
            continue;
        }

        if let Ok(name) = file_name.into_string() {
            projects.push(name);
        }
    }

    projects.sort();

    if projects.is_empty() {
        bail!("No projects found in: {}", base_path.display());
    }

    Ok(projects)
}

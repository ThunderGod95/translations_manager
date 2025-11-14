use std::path::Path;

use anyhow::{Context, Result, bail};
use console::style;
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Confirm, Select};
use jwalk::WalkDir;

use crate::cache::*;

pub fn select_project(base_path: impl AsRef<Path>) -> Result<String> {
    let base_path = base_path.as_ref();
    let projects = get_projects(base_path)?;

    if projects.len() == 1 {
        let project_name = projects.first().unwrap().clone();
        println!("\nOnly one project found. Auto-selecting: {}", project_name);

        if let Err(e) = write_cache(Cache {
            last_project: project_name.clone(),
        }) {
            println!(
                "{}",
                style(format!("[WARN] Could not write to cache file: {}", e)).yellow()
            );
        }

        return Ok(project_name);
    }

    if let Some(cache) = read_cache()? {
        let last_project = cache.last_project;

        if projects.binary_search(&last_project).is_ok() {
            let use_last = Confirm::with_theme(&ColorfulTheme::default())
                .with_prompt(format!("Use last selected project: {}", &last_project))
                .default(true)
                .show_default(true)
                .interact()
                .context("Failed to render confirmation prompt")?;

            if use_last {
                println!("\nUsing cached project: {}\n", &last_project);
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
    }) {
        println!(
            "{}",
            style(format!("[WARN] Could not write to cache file: {}", e)).yellow()
        );
    }

    Ok(selected_project)
}

pub fn get_projects(base_path: &Path) -> Result<Vec<String>> {
    if !base_path.is_dir() {
        bail!(
            "Failed to read projects: {} is not a valid directory",
            base_path.display()
        );
    }

    let mut projects: Vec<_> = WalkDir::new(base_path)
        .max_depth(1)
        .into_iter()
        .filter_map(|entry_result| {
            let entry = entry_result.ok()?;

            if entry.depth() == 0 {
                return None;
            }

            let file_name = entry.file_name();
            if !entry.file_type().is_dir() || file_name == "tscripts" {
                return None;
            }

            file_name.to_str().map(String::from)
        })
        .collect();

    projects.sort();

    if projects.is_empty() {
        bail!("No projects found in: {}", base_path.display());
    }

    Ok(projects)
}

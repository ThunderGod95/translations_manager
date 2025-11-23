use std::path::Path;
use std::{env, fs};

use anyhow::{Context, Result, bail};
use console::style;
use dialoguer::FuzzySelect;
use dialoguer::theme::ColorfulTheme;

use crate::cache::*;

pub fn select_project(base_path: impl AsRef<Path>) -> Result<String> {
    let base_path = base_path.as_ref();
    let projects = get_projects(base_path)?;

    if let Ok(cwd) = env::current_dir() {
        let base = cwd.file_name().unwrap().to_str().unwrap().to_owned();

        if projects.contains(&base) {
            println!(
                "Detected application running in known project. Auto-selecting: {}",
                base
            );
            return Ok(base);
        }
    }

    let mut cache = read_cache()?.unwrap_or_default();

    if projects.len() == 1 {
        let project_name = projects.first().unwrap().clone();
        println!("\nOnly one project found. Auto-selecting: {}", project_name);

        cache.last_project = project_name.clone();

        if let Err(e) = write_cache(cache) {
            println!(
                "{}",
                style(format!("[WARN] Could not write to cache file: {}", e)).yellow()
            );
        }

        return Ok(project_name);
    }

    let default_index = if !cache.last_project.is_empty() {
        projects
            .iter()
            .position(|p| p == &cache.last_project)
            .unwrap_or(0)
    } else {
        0
    };

    let project_index = FuzzySelect::with_theme(&ColorfulTheme::default())
        .with_prompt("Select a translation project")
        .items(&projects)
        .default(default_index)
        .interact()
        .context("Failed to render selection prompt")?;

    let selected_project = projects[project_index].clone();

    cache.last_project = selected_project.clone();

    if let Err(e) = write_cache(cache) {
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

    let mut projects: Vec<String> = fs::read_dir(base_path)?
        .filter_map(|entry| {
            let entry = entry.ok()?;
            let path = entry.path();

            if path.is_dir() && path.file_name()?.to_str()? != "tscripts" {
                return Some(path.file_name()?.to_str()?.to_string());
            }
            None
        })
        .collect();

    projects.sort();

    if projects.is_empty() {
        bail!("No projects found in: {}", base_path.display());
    }

    Ok(projects)
}

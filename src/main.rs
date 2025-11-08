use anyhow::{Context, Result, anyhow, bail};
use clap::Parser;
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Confirm, Select};
use directories::BaseDirs;
use itertools::Itertools;
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::{fs, process};
use strum::VariantArray;

use crate::runner::cli::Task;
use crate::runner::*;
use crate::util::{get_cache_path, wait_for_input_if_standalone};

pub mod config;
pub mod distribute;
pub mod find;
pub mod glossary;
pub mod init;
pub mod replace;
pub mod runner;
pub mod util;

async fn run_app(cli: Cli) -> Result<()> {
    let base_path = if let Some(path) = cli.path {
        path.canonicalize()
            .with_context(|| format!("Failed to find projects directory at: {}", path.display()))?
    } else {
        let base_dirs = BaseDirs::new().unwrap();
        let user_dir = base_dirs.home_dir();
        user_dir.join("Translations")
    };

    let project = if let Some(project_name) = cli.project {
        let projects = get_projects(&base_path)?;
        if !projects.contains(&project_name) {
            bail!(
                "Project '{}' not found in {}",
                project_name,
                base_path.display()
            );
        }
        info!("✅ Using project from argument: {}", project_name);
        Some(project_name)
    } else {
        None
    };

    let mut selected_task: Command = if let Some(command) = cli.command {
        command
    } else {
        let task_index = Select::with_theme(&ColorfulTheme::default())
            .items(Task::VARIANTS)
            .default(0)
            .with_prompt("Select a task to run:")
            .interact_opt()?;

        if let Some(task_index) = task_index {
            let task = Task::VARIANTS[task_index];

            Command::from_task(task)
        } else {
            return Err(anyhow!("No task selected. Exiting."));
        }
    };

    handle_task(&mut selected_task, base_path, project).await?;

    Ok(())
}

async fn handle_task(
    task: &mut Command,
    base_path: impl AsRef<Path>,
    project: Option<impl AsRef<Path>>,
) -> Result<()> {
    populate_arguments(task)?;

    // First run tasks that don't need project path.
    match task {
        Command::Internal => {
            return run_internal_task().await;
        }
        Command::Init(args) => {
            return run_init_task(args, base_path.as_ref()).await;
        }
        _ => {}
    }

    let project_path = if let Some(project) = project {
        base_path.as_ref().join(project)
    } else {
        let project = select_project(&base_path)?;
        base_path.as_ref().join(project)
    };

    match task {
        Command::Glossary => run_glossary_task(&project_path).await?,
        Command::Find(args) => {
            run_find_task(args, &project_path).await?;
        }
        Command::Replace(args) => {
            run_replace_task(args, &project_path).await?;
        }
        Command::Distribute => run_dist_task(&project_path).await?,
        Command::Open(args) => {
            run_open_task(args, &project_path).await;
        }
        Command::Internal | Command::Init(_) => {
            unreachable!("Pathless commands should have been handled by the guard match")
        }
    }

    Ok(())
}

fn select_project(base_path: impl AsRef<Path>) -> Result<String> {
    let base_path = base_path.as_ref();
    let projects = get_projects(base_path)?;

    if projects.len() == 1 {
        let project_name = projects.first().unwrap().clone();
        info!(
            "\n✅ Only one project found. Auto-selecting: {}",
            project_name
        );

        if let Err(e) = write_cache(Cache {
            last_project: project_name.clone(),
        }) {
            warn!("Warning: Could not write to cache file: {}", e);
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
    }) {
        warn!("Warning: Could not write to cache file: {}", e);
    }

    Ok(selected_project)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Cache {
    #[serde(rename = "lastProject")]
    last_project: String,
}

fn read_cache() -> Result<Option<Cache>> {
    let cache_path = get_cache_path()?;
    if !cache_path.exists() {
        return Ok(None);
    }

    let cache_string = fs::read_to_string(&cache_path)
        .with_context(|| format!("Failed to read cache file at {}", cache_path.display()))?;

    let cache: Cache = serde_json::from_str(&cache_string)
        .with_context(|| format!("Failed to parse cache file at {}", cache_path.display()))?;

    Ok(Some(cache))
}

fn write_cache(cache: Cache) -> Result<()> {
    let cache_path = get_cache_path()?;
    let j = serde_json::to_string_pretty(&cache).context("Failed to serialize cache")?;

    fs::write(&cache_path, j)
        .with_context(|| format!("Failed to write cache file to {}", cache_path.display()))?;

    Ok(())
}

fn get_projects(base_path: &Path) -> Result<Vec<String>> {
    let projects: Vec<String> = fs::read_dir(base_path)
        .with_context(|| format!("Failed to read projects from {}", base_path.display()))?
        .filter_map(|entry_result| {
            let entry = entry_result.ok()?;
            let path = entry.path();

            if !path.is_dir() || entry.file_name() == "tscripts" {
                return None;
            }

            entry.file_name().into_string().ok()
        })
        .sorted()
        .collect();

    if projects.is_empty() {
        bail!("No projects found in: {}", base_path.display());
    }

    Ok(projects)
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    env_logger::Builder::new()
        .filter_level(cli.verbose.log_level_filter())
        .format_timestamp(None)
        .format_file(false)
        .format_module_path(false)
        .init();

    if let Err(e) = run_app(cli).await {
        error!("{}", e);
        process::exit(1);
    }

    wait_for_input_if_standalone().await;
}

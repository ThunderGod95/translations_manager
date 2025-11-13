use anyhow::{Context, Result, anyhow, bail};
use clap::Parser;
use dialoguer::Select;
use dialoguer::theme::ColorfulTheme;
use directories::BaseDirs;
use log::{error, info};
use std::path::Path;
use std::process;
use strum::VariantArray;

use crate::projects::*;
use crate::runner::cli::Task;
use crate::runner::*;
use crate::util::{is_standalone, prompt_for_rerun};

pub mod cache;
pub mod config;
pub mod distribute;
pub mod find;
pub mod glossary;
pub mod init;
pub mod projects;
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
        let projects = get_projects(&base_path).await?;
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

    let selected_task: Command = if let Some(command) = cli.command {
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

    handle_task(selected_task, base_path, project).await?;

    Ok(())
}

async fn handle_task(
    task: Command,
    base_path: impl AsRef<Path>,
    project: Option<impl AsRef<Path>>,
) -> Result<()> {
    let task = populate_arguments(task).await?;

    // First run tasks that don't need project path.
    match task {
        Command::Internal => {
            return run_internal_task().await;
        }
        Command::Init(args) => {
            return run_init_task(&args, base_path.as_ref()).await;
        }
        _ => {}
    }

    let project_path = if let Some(project) = project {
        base_path.as_ref().join(project)
    } else {
        let project = select_project(&base_path).await?;
        base_path.as_ref().join(project)
    };

    match task {
        Command::Glossary => run_glossary_task(&project_path).await?,
        Command::Find(args) => {
            run_find_task(&args, &project_path).await?;
        }
        Command::Replace(args) => {
            run_replace_task(&args, &project_path).await?;
        }
        Command::Distribute(args) => run_dist_task(&args, &project_path).await?,
        Command::Open(args) => {
            run_open_task(&args, &project_path).await;
        }
        Command::Internal | Command::Init(_) => {
            unreachable!("Pathless commands should have been handled by the guard match")
        }
    }

    Ok(())
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

    let standalone = is_standalone();

    let mut run_result = run_app(cli.clone()).await;

    if standalone {
        loop {
            if let Err(e) = run_result {
                error!("{}", e);
            }

            if !prompt_for_rerun().await {
                break;
            }

            run_result = run_app(cli.clone()).await;
        }
    } else {
        if let Err(e) = run_result {
            error!("{}", e);
            process::exit(1);
        }
    }
}

use anyhow::{Context, Result, anyhow, bail};
use clap::Parser;
use console::style;
use dialoguer::Select;
use dialoguer::theme::ColorfulTheme;
use directories::BaseDirs;
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

fn run_app(cli: Cli) -> Result<()> {
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
        println!("Using project from argument: {}", project_name);
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

    handle_task(&mut selected_task, base_path, project)?;

    Ok(())
}

fn handle_task(
    task: &mut Command,
    base_path: impl AsRef<Path>,
    project: Option<impl AsRef<Path>>,
) -> Result<()> {
    populate_arguments(task)?;

    // First run tasks that don't need project path.
    match task {
        Command::Internal => {
            return run_internal_task();
        }
        Command::Init(args) => {
            return run_init_task(&args, base_path.as_ref());
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
        Command::Glossary => run_glossary_task(&project_path)?,
        Command::Find(args) => {
            run_find_task(&args, &project_path)?;
        }
        Command::Replace(args) => {
            run_replace_task(&args, &project_path)?;
        }
        Command::Distribute(args) => run_dist_task(&args, &project_path)?,
        Command::Open(args) => {
            run_open_task(&args, &project_path);
        }
        Command::Internal | Command::Init(_) => {
            unreachable!("Pathless commands should have been handled by the guard match")
        }
    }

    Ok(())
}

fn main() {
    let cli = Cli::parse();

    let standalone = is_standalone();

    let mut run_result = run_app(cli.clone());

    if standalone {
        loop {
            if let Err(e) = run_result {
                eprintln!("{}", style(format!("[ERROR] {}", e)).red());
            }

            if !prompt_for_rerun() {
                break;
            }

            run_result = run_app(cli.clone());
        }
    } else {
        if let Err(e) = run_result {
            eprintln!("{}", style(format!("[ERROR] {}", e)).red());
            process::exit(1);
        }
    }
}

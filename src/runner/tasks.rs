use std::{
    fs::{self},
    path::Path,
};

use anyhow::{Result, bail};
use dialoguer::{Confirm, theme::ColorfulTheme};
use strum::VariantArray;

use super::cli::*;
use crate::{
    clean,
    config::CONFIG,
    distribute::*,
    editor,
    glossary::{find_last_chapter, glossary_processor},
    init::*,
    runner::cli::DistArgs,
    util::{get_cache_path, get_config_file_path},
};

pub fn run_glossary_task(project_name: &str) -> Result<()> {
    glossary_processor(project_name, true)?;

    Ok(())
}

pub fn run_internal_task() -> Result<()> {
    let config_path = get_config_file_path()?;
    let cache_path = get_cache_path()?;

    editor::open_in_editor(&[config_path, cache_path])?;

    Ok(())
}

pub fn run_dist_task(args: &DistArgs, project_name: &str) -> Result<()> {
    println!("Running 'distribute' on {project_name}");

    let distributor = Distributor::new(project_name)?;

    let mut volumes = args.volumes.clone();
    volumes.sort();
    volumes.dedup();

    if args.formats.is_empty() {
        for format in DistributionFormat::VARIANTS {
            distributor.add(*format, &volumes)?;
        }
    } else {
        let mut formats = args.formats.clone();
        formats.sort();
        formats.dedup();

        for format in &formats {
            distributor.add(*format, &volumes)?;
        }
    }

    let result = distributor.wait();

    if let Err(e) = result {
        bail!("Task failed. Reason: {}", e);
    }

    Ok(())
}

pub fn run_open_task(open_args: &OpenArgs, project_path: &Path) -> Result<()> {
    let chapter_nos = open_args.files.as_ref();

    if let Some(chapter_nos) = chapter_nos {
        let chapter_paths: Vec<_> = chapter_nos
            .iter()
            .map(|no| {
                project_path
                    .join(&CONFIG.read().unwrap().translations_folder)
                    .join(format!("{}.md", no))
            })
            .collect();

        editor::open_in_editor(&chapter_paths)?;
    } else {
        editor::open_in_editor(&[project_path])?;
    }

    Ok(())
}

pub fn run_init_task(init_args: &InitArgs, base_path: &Path) -> Result<()> {
    let project_name = init_args.project_name.clone().unwrap();
    let project_path = base_path.join(&project_name);

    println!("\nCreating new project at: {}", project_path.display());

    write_embedded_dir(&TEMPLATE_DIR, project_path)?;

    println!("\nSuccessfully created: {}", project_name);

    Ok(())
}

pub fn run_next_task(project_name: &str, project_path: &Path) -> Result<()> {
    let config = CONFIG.read().expect("Failed to acquire config lock");

    let raws_dir = project_path.join(&config.raws_folder);

    if !raws_dir.exists() {
        bail!("'raws' folder not found. Please directly use the `glossary` command.");
    }

    let assets_dir = project_path.join(&config.assets_folder);
    let translations_dir = project_path.join(&config.translations_folder);

    let last_chapter = find_last_chapter(&translations_dir)?;

    println!("Last Chapter: {}", last_chapter);
    println!("Next Chapter: {}", last_chapter + 1);

    let next_chapter_path = raws_dir.join(format!("{}.md", last_chapter + 1));
    let cr_ch_path = assets_dir.join(&config.chapter_file);

    if let Err(e) = fs::copy(&next_chapter_path, &cr_ch_path) {
        let error_msg = if e.kind() == std::io::ErrorKind::NotFound {
            format!("Next chapter file not found: {:?}", next_chapter_path)
        } else {
            format!("System error copying chapter: {}", e)
        };

        let confirm = Confirm::with_theme(&ColorfulTheme::default())
            .default(true)
            .show_default(true)
            .with_prompt(format!(
                "{} \nDo you still want to proceed with the existing chapter?",
                error_msg
            ))
            .interact()?;

        if !confirm {
            return Ok(());
        }
    }

    glossary_processor(project_name, false)?;

    Ok(())
}

pub fn run_clean_task(args: &CleanArgs, project: &String) -> Result<()> {
    if let Some(_) = args.file {
        bail!("Single file cleaning is not implemented yet.")
    } else {
        clean::clean_project(&project, args.yaml, args.pad)
    }
}

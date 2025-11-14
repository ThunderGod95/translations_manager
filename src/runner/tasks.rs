//! Contains the core business logic for executing each command.

use std::{
    fs::File,
    io::BufWriter,
    path::{Path, PathBuf},
};

use anyhow::{Result, anyhow, bail};
use console::style;
use strum::VariantArray;

use super::cli::*;
use crate::{
    config::CONFIG,
    distribute::*,
    find::*,
    glossary::glossary_processor,
    init::*,
    replace::replace_in_folder,
    runner::cli::DistArgs,
    util::{get_cache_path, get_config_file_path, open_in_vs_code},
};

pub fn run_glossary_task(project_path: &PathBuf) -> Result<()> {
    let assets_path = project_path.join(&CONFIG.assets_folder);
    let translations_path = project_path.join(&CONFIG.translations_folder);

    glossary_processor(assets_path, translations_path)?;

    Ok(())
}

pub fn run_find_task(args: &FindArgs, project_path: &PathBuf) -> Result<()> {
    let FindArgs {
        pattern,
        regex: use_regex,
        start: start_file,
        end: end_file,
        write: write_path,
        silent,
    } = args;

    let folder_path = project_path.join(&CONFIG.translations_folder);
    let search_pattern = pattern.as_ref().unwrap();
    let write_path = write_path.as_ref();

    let matches = find_single_in_folder(
        &folder_path,
        &search_pattern,
        *use_regex,
        *start_file,
        *end_file,
    )?;

    if matches.is_empty() {
        println!("No match found in any file in: {}", folder_path.display());
        return Ok(());
    }

    if !silent {
        let mut buffer: Vec<u8> = Vec::new();
        format_folder_matches(&matches, FormatOption::Table, &mut buffer)?;

        let output_string = String::from_utf8_lossy(&buffer);
        println!("{}", output_string);
    }

    if let Some(path) = write_path {
        let file_path = project_path.join(path);
        let file = File::create(&file_path)?;
        let writer = BufWriter::new(file);

        format_folder_matches(&matches, FormatOption::Paragraphs, writer)?;

        println!("Successfully wrote matches to: {}", file_path.display());
    }

    let first_chapter_match = matches.first().unwrap().0;
    println!("Found first match in -> Chapter {}", first_chapter_match);

    let total_matches: usize = matches.iter().map(|(_, m)| m.len()).sum();
    println!("Total matches found -> {}", total_matches);

    Ok(())
}

pub fn run_internal_task() -> Result<()> {
    let config_path = get_config_file_path()?;
    let cache_path = get_cache_path()?;

    open_in_vs_code(&[config_path, cache_path]);

    Ok(())
}

pub fn run_dist_task(args: &DistArgs, project_path: &Path) -> Result<()> {
    println!("Running 'distribute' on {}", project_path.display());

    let translations_dir = project_path.join(&CONFIG.translations_folder);
    let assets_dir = project_path.join(&CONFIG.assets_folder);
    let dist_dir = project_path.join(&CONFIG.dist_folder);

    let formats_to_build = if args.txt {
        println!(
            "{}",
            style(format!(
                "[WARN] Distributing as TXT. Disabling other formats..."
            ))
            .yellow()
        );
        &[DistributionFormat::TXT]
    } else {
        DistributionFormat::VARIANTS
    };

    let results: Vec<Result<()>> = formats_to_build
        .iter()
        .map(|&format| distribute(format, &translations_dir, &assets_dir, &dist_dir))
        .collect();

    for (format, result) in formats_to_build.iter().zip(results.iter()) {
        if let Err(e) = result {
            bail!("{} creation failed. Reason: {}", format, e);
        }
    }

    Ok(())
}

pub fn run_replace_task(replace_args: &ReplaceArgs, project_path: &Path) -> Result<()> {
    let ReplaceArgs { old, new, regex } = replace_args;

    let old_text = old
        .as_deref()
        .ok_or_else(|| anyhow!("Missing required 'old' argument"))?;

    let new_text = new
        .as_deref()
        .ok_or_else(|| anyhow!("Missing required 'new' argument"))?;

    let translations_dir = project_path.join(&CONFIG.translations_folder);

    let (total_replacements, total_files_updated) =
        replace_in_folder(translations_dir, old_text, new_text, *regex)?;

    let separator = "=".repeat(50);

    println!("\n\n{}", separator);
    println!("Total {} files updated.", total_files_updated);
    println!("Total {} replacements made.", total_replacements);
    println!("{}", separator);

    Ok(())
}

pub fn run_open_task(open_args: &OpenArgs, project_path: &Path) {
    let chapter_nos = open_args.files.as_ref();

    if let Some(chapter_nos) = chapter_nos {
        let chapter_paths: Vec<_> = chapter_nos
            .iter()
            .map(|no| {
                project_path
                    .join(&CONFIG.translations_folder)
                    .join(format!("{}.md", no))
            })
            .collect();

        open_in_vs_code(&chapter_paths);
    } else {
        open_in_vs_code(&[project_path]);
    }
}

pub fn run_init_task(init_args: &InitArgs, base_path: &Path) -> Result<()> {
    let project_name = init_args.project_name.clone().unwrap();
    let project_path = base_path.join(&project_name);

    println!("\nCreating new project at: {}", project_path.display());

    write_embedded_dir(&TEMPLATE_DIR, project_path)?;

    println!("\nSuccessfully created: {}", project_name);

    Ok(())
}

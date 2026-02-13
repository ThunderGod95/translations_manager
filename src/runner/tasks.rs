use std::{
    fs::{self, File},
    io::BufWriter,
    path::Path,
};

use anyhow::{Result, anyhow, bail};
use dialoguer::{Confirm, theme::ColorfulTheme};
use strum::VariantArray;

use super::cli::*;
use crate::{
    clean,
    config::CONFIG,
    distribute::*,
    editor,
    find::*,
    glossary::{find_last_chapter, glossary_processor},
    init::*,
    replace::replace_in_folder,
    runner::cli::DistArgs,
    util::{get_cache_path, get_config_file_path},
};

pub fn run_glossary_task(project_name: &str) -> Result<()> {
    glossary_processor(project_name, true)?;

    Ok(())
}

pub fn run_find_task(args: &FindArgs, project_name: &str, project_path: &Path) -> Result<()> {
    let FindArgs {
        pattern,
        regex: use_regex,
        start: start_file,
        end: end_file,
        write: write_path,
        silent,
    } = args;

    let folder_path = project_path.join(&CONFIG.read().unwrap().translations_folder);
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
        println!("No match found in any chapter in project: {}", project_name);
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

pub fn run_replace_task(replace_args: &ReplaceArgs, project_path: &Path) -> Result<()> {
    let ReplaceArgs { old, new, regex } = replace_args;

    let old_text = old
        .as_deref()
        .ok_or_else(|| anyhow!("Missing required 'old' argument"))?;

    let new_text = new
        .as_deref()
        .ok_or_else(|| anyhow!("Missing required 'new' argument"))?;

    let translations_dir = project_path.join(&CONFIG.read().unwrap().translations_folder);

    let (total_replacements, total_files_updated) =
        replace_in_folder(translations_dir, old_text, new_text, *regex)?;

    let separator = "=".repeat(50);

    println!("\n\n{}", separator);
    println!("Total {} files updated.", total_files_updated);
    println!("Total {} replacements made.", total_replacements);
    println!("{}", separator);

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

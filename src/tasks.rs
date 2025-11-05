use std::{
    fs::File,
    io::BufWriter,
    path::{Path, PathBuf},
};

use anyhow::{Result, anyhow};
use futures::future::join_all;

use crate::{
    commands::{FindArgs, OpenArgs, ReplaceArgs},
    config::CONFIG,
    dist::{DistributionFormat, distribute},
    find::single::{FormatOption, find_matches_in_folder, format_folder_matches},
    glossary::GlossaryProcessor,
    replace::replace_in_folder,
    util::{
        get_cache_path, get_config_file_path, log_error, log_info, log_success, open_in_vs_code,
    },
};

pub async fn run_glossary_task(project_path: &PathBuf) -> Result<()> {
    let assets_path = project_path.join(&CONFIG.assets_folder);
    let translations_path = project_path.join(&CONFIG.translations_folder);

    let glossary_processor = GlossaryProcessor::new(assets_path, translations_path)?;

    glossary_processor.process_new_chapters().await?;

    Ok(())
}

pub async fn run_find_task(args: &FindArgs, project_path: &PathBuf) -> Result<()> {
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

    let matches = find_matches_in_folder(
        &folder_path,
        &search_pattern,
        *use_regex,
        *start_file,
        *end_file,
    )?;

    if matches.is_empty() {
        log_info(format!(
            "No match found in any file in: {}",
            folder_path.display()
        ));
        return Ok(());
    }

    if !silent {
        let mut buffer: Vec<u8> = Vec::new();
        format_folder_matches(&matches, FormatOption::Table, &mut buffer)?;

        let output_string = String::from_utf8_lossy(&buffer);
        log_info(output_string);
    }

    if let Some(path) = write_path {
        let file_path = project_path.join(path);
        let file = File::create(&file_path)?;
        let writer = BufWriter::new(file);

        format_folder_matches(&matches, FormatOption::Paragraphs, writer)?;

        log_info(format!(
            "Successfully wrote matches to: {}",
            file_path.display()
        ));
    }

    let find_file_match = matches.keys().next().unwrap().file_name().unwrap();
    log_info(format!(
        "Found first match in: {}",
        find_file_match.display()
    ));

    let total_matches: usize = matches.values().map(Vec::len).sum();
    log_info(format!("Total matches found: {}", total_matches));

    Ok(())
}

pub async fn run_internal_task() -> Result<()> {
    let config_path = get_config_file_path()?;
    let cache_path = get_cache_path()?;

    open_in_vs_code(&[config_path, cache_path]).await;

    Ok(())
}

pub async fn run_dist_task(project_path: &Path) -> Result<()> {
    println!("Running 'distribute' on {}", project_path.display());

    let translations_dir = project_path.join(&CONFIG.translations_folder);
    let assets_dir = project_path.join(&CONFIG.assets_folder);
    let dist_dir = project_path.join(&CONFIG.dist_folder);

    let formats_to_build = [DistributionFormat::PDF, DistributionFormat::EPUB];

    let dist_tasks = formats_to_build
        .iter()
        .map(|&format| distribute(format, &translations_dir, &assets_dir, &dist_dir));

    let results = join_all(dist_tasks).await;

    for (format, result) in formats_to_build.iter().zip(results.iter()) {
        if let Err(e) = result {
            log_error(format!("{} creation failed.", format));
            log_error(e);
        }
    }

    Ok(())
}

pub async fn run_replace_task(replace_args: &ReplaceArgs, project_path: &Path) -> Result<()> {
    let ReplaceArgs { old, new, regex } = replace_args;

    let old_text = old
        .as_deref()
        .ok_or_else(|| anyhow!("Missing required 'old' argument"))?;

    let new_text = new
        .as_deref()
        .ok_or_else(|| anyhow!("Missing required 'new' argument"))?;

    let translations_dir = project_path.join(&CONFIG.translations_folder);

    let (total_replacements, total_files_updated) =
        replace_in_folder(translations_dir, old_text, new_text, *regex).await?;

    let separator = "=".repeat(50);

    log_success(format!("\n{}", separator));
    log_success(format!("\nTotal {} files updated.", total_files_updated));
    log_success(format!("Total {} replacements made.", total_replacements));
    log_success(format!("{}", separator));

    Ok(())
}

pub async fn run_open_task(open_args: &OpenArgs, project_path: &Path) {
    let chapter_nos = open_args.files.as_ref();

    if let Some(chapter_nos) = chapter_nos {
        let chapter_paths: Vec<_> = chapter_nos
            .iter()
            .map(|no| {
                project_path
                    .join(&CONFIG.translations_folder) // Default is "translations"
                    .join(format!("{}.md", no))
            })
            .collect();

        open_in_vs_code(&chapter_paths).await;
    } else {
        open_in_vs_code(&[project_path]).await;
    }
}

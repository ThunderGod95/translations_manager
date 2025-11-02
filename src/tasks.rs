use std::{fs::File, io::BufWriter, path::PathBuf};

use anyhow::Result;

use crate::{
    commands::FindArgs,
    config::CONFIG,
    find::single::{FormatOption, find_matches_in_folder, format_folder_matches},
    glossary::GlossaryProcessor,
    util::{get_config_file_path, log_info, open_in_vs_code},
};

pub fn run_glossary_task(project_path: &PathBuf) -> Result<()> {
    let assets_path = project_path.join(&CONFIG.assets_folder);
    let translations_path = project_path.join(&CONFIG.translations_folder);

    let glossary_processor = GlossaryProcessor::new(assets_path, translations_path)?;

    glossary_processor.process_new_chapters()?;

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

pub fn run_config_task() -> Result<()> {
    let config_path = get_config_file_path()?;

    open_in_vs_code(&[config_path]);

    Ok(())
}

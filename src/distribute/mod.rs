mod input;
mod pandoc;

use std::fs::{self, create_dir_all, remove_dir_all};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result};
use console::style;
use serde::{Deserialize, Serialize};
use strum::{Display, VariantArray};
use threadpool::ThreadPool;

use crate::config::CONFIG;

use self::input::build_input;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VolumeInfo {
    #[serde(rename = "filestart")]
    pub file_start: usize,
    #[serde(rename = "fileend")]
    pub file_end: usize,
    position: usize,
    title: String,
    series: String,
    author: String,
    translator: String,
    rights: String,
    image: String,
}

impl VolumeInfo {
    pub fn load(from: impl AsRef<Path>) -> Result<Vec<Self>> {
        let path = from.as_ref();
        let mut content = fs::read(path).with_context(|| {
            format!(
                "Failed to read the volume info file: '{}'.\n  \
                 Please ensure this file exists and you have read permissions.",
                path.display()
            )
        })?;

        let data = simd_json::from_slice(&mut content).with_context(|| {
            format!(
                "Failed to parse the volume info file: '{}'.\n  \
                 The file seems to be corrupted or is not valid JSON.",
                path.display()
            )
        })?;

        Ok(data)
    }
}

#[derive(Debug, Clone, Copy, Display, PartialEq, Eq, VariantArray)]
pub enum DistributionFormat {
    EPUB,
    PDF,
    TXT,
}

pub fn distribute(
    dist_format: DistributionFormat,
    translations_dir: impl AsRef<Path>,
    assets_dir: impl AsRef<Path>,
    dist_dir: impl AsRef<Path>,
) -> Result<()> {
    let translations_dir = translations_dir.as_ref().to_path_buf();
    let assets_dir = assets_dir.as_ref().to_path_buf();
    let output_sub_dir = prepare_output_directory(&dist_dir, dist_format)?;

    let volumes = VolumeInfo::load(assets_dir.join(&CONFIG.sep_info_file))
        .context("Failed to load volume configuration. Check your assets directory.")?;

    println!("Creating {}s... (Total: {})", dist_format, volumes.len());

    let pool = ThreadPool::new(num_cpus::get());
    let results = Arc::new(Mutex::new(Vec::new()));

    for vol in volumes {
        let results = Arc::clone(&results);
        let translations_dir = translations_dir.clone();
        let assets_dir = assets_dir.clone();
        let output_sub_dir = output_sub_dir.clone();

        pool.execute(move || {
            let res = process_volume(
                &vol,
                dist_format,
                &translations_dir,
                &assets_dir,
                &output_sub_dir,
            );

            results.lock().unwrap().push(res);
        });
    }

    pool.join();

    let mutex = Arc::try_unwrap(results).expect(
        "A critical internal error occurred while collecting thread results. \
         This indicates a bug in the application. (ARC_UNWRAP_FAILED)",
    );

    let results = match mutex.into_inner() {
        Ok(results) => results,
        Err(poison_error) => {
            eprintln!(
                "{}",
                style(
                    "[WARN] One or more processing threads crashed. \
                     Results may be incomplete."
                )
                .yellow()
            );
            poison_error.into_inner()
        }
    };

    log_errors(dist_format, results)
}

fn process_volume(
    vol: &VolumeInfo,
    dist_format: DistributionFormat,
    translations_dir: &Path,
    assets_dir: &Path,
    output_dir: &Path,
) -> Result<()> {
    println!("Creating {} for {}", dist_format, &vol.title);

    let output_file_name = sanitize_filename::sanitize(&vol.title);
    let output_file_path = output_dir.join(format!(
        "{}.{}",
        output_file_name,
        dist_format.to_string().to_lowercase()
    ));

    let metadata = pandoc::build_metadata(vol, assets_dir)
        .with_context(|| format!("Failed to prepare metadata for volume: '{}'", vol.title))?;

    let input = build_input(
        translations_dir,
        vol,
        dist_format,
        metadata.get_cover_image().as_deref(),
    )
    .with_context(|| format!("Failed to gather text content for volume: '{}'", vol.title))?;

    let args = pandoc::build_args(
        dist_format,
        &metadata,
        translations_dir,
        assets_dir,
        &output_file_path,
    )
    .with_context(|| {
        format!(
            "Failed to build conversion arguments for volume: '{}'",
            vol.title
        )
    })?;

    if dist_format == DistributionFormat::TXT {
        distribute_as_txt(input, output_file_path)
            .with_context(|| format!("Failed to write TXT file for volume: '{}'", vol.title))
    } else {
        pandoc::run(args, input)
            .with_context(|| format!("Pandoc failed while converting volume: '{}'", vol.title))
    }
}

fn prepare_output_directory(
    dist_dir: impl AsRef<Path>,
    format: DistributionFormat,
) -> Result<PathBuf> {
    let sub_dir_name = format!("{}s", format.to_string().to_lowercase());
    let output_sub_dir = dist_dir.as_ref().join(sub_dir_name);

    if output_sub_dir.exists() {
        remove_dir_all(&output_sub_dir).with_context(|| {
            format!(
                "Failed to clear the old output directory: '{}'.\n  \
                 Check if any files inside are in use by another program.",
                output_sub_dir.display()
            )
        })?;
    }

    create_dir_all(&output_sub_dir).with_context(|| {
        format!(
            "Failed to create the output directory: '{}'.\n  \
             Please check if you have write permissions for this location.",
            output_sub_dir.display()
        )
    })?;

    Ok(output_sub_dir)
}

fn log_errors(dist_format: DistributionFormat, results: Vec<Result<()>>) -> Result<()> {
    let total = results.len();
    let mut errors = Vec::with_capacity(results.len());
    let mut successes = 0;

    for result in results {
        match result {
            Ok(_) => successes += 1,
            Err(e) => errors.push(e),
        }
    }

    println!(
        "{}",
        style(format!(
            "Successfully processed {}/{} {}s.",
            successes, total, dist_format
        ))
        .green(),
    );

    if errors.is_empty() {
        Ok(())
    } else {
        eprintln!(
            "{}",
            style(format!(
                "Failed to process {}/{} {}:",
                errors.len(),
                total,
                dist_format
            ))
            .red()
        );

        for (i, e) in errors.iter().enumerate() {
            eprintln!("{}", style(format!("  Failure {}: {:#}", i + 1, e)).red())
        }

        Err(errors.remove(0))
    }
}

fn distribute_as_txt(input: String, output_file: impl AsRef<Path>) -> Result<()> {
    fs::write(output_file.as_ref(), input).with_context(|| {
        format!(
            "Failed to write content to TXT file: '{}'.\n  \
             Please check your disk space and write permissions.",
            output_file.as_ref().display()
        )
    })?;

    Ok(())
}

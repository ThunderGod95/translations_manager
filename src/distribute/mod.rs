mod input;
mod pandoc;

use anyhow::{Context, Result};
use futures::future::join_all;
use log::info;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use strum::{Display, VariantArray};
use tokio::fs::{self, create_dir_all, read_to_string, remove_dir_all};

use crate::config::get_config;

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
    pub async fn load(from: impl AsRef<Path>) -> Result<Vec<Self>> {
        let content = read_to_string(from)
            .await
            .context("Failed to read volume separation file")?;

        let data =
            serde_json::from_str(&content).context("Failed to parse volume separation info.")?;

        Ok(data)
    }
}

#[derive(Debug, Clone, Copy, Display, PartialEq, Eq, VariantArray)]
pub enum DistributionFormat {
    EPUB,
    PDF,
    TXT,
}

pub async fn distribute(
    dist_format: DistributionFormat,
    translations_dir: impl AsRef<Path>,
    assets_dir: impl AsRef<Path>,
    dist_dir: impl AsRef<Path>,
) -> Result<()> {
    let translations_dir = translations_dir.as_ref();
    let assets_dir = assets_dir.as_ref();
    let output_sub_dir = prepare_output_directory(&dist_dir, dist_format).await?;

    let volumes = VolumeInfo::load(assets_dir.join(&get_config().await.sep_info_file)).await?;

    info!("Creating {}s... (Total: {})", dist_format, volumes.len());

    let futures: Vec<_> = volumes
        .iter()
        .map(|vol| {
            process_volume(
                vol,
                dist_format,
                translations_dir,
                assets_dir,
                &output_sub_dir,
            )
        })
        .collect();

    let results = join_all(futures).await;

    log_errors(dist_format, results)
}

async fn process_volume(
    vol: &VolumeInfo,
    dist_format: DistributionFormat,
    translations_dir: &Path,
    assets_dir: &Path,
    output_dir: &Path,
) -> Result<()> {
    info!("Creating {} for {}", dist_format, &vol.title);

    let output_file_name = sanitize_filename::sanitize(&vol.title);
    let output_file_path = output_dir.join(format!(
        "{}.{}",
        output_file_name,
        dist_format.to_string().to_lowercase()
    ));

    let metadata = pandoc::build_metadata(&vol, &assets_dir)?;

    let input = build_input(
        &translations_dir,
        &vol,
        dist_format,
        metadata.get_cover_image().as_deref(),
    )
    .await?;

    let args = pandoc::build_args(
        dist_format,
        &metadata,
        &translations_dir,
        &assets_dir,
        &output_file_path,
    )
    .await?;

    if dist_format == DistributionFormat::TXT {
        distribute_as_txt(input, output_file_path).await
    } else {
        pandoc::run(args, input).await
    }
}

async fn prepare_output_directory(
    dist_dir: impl AsRef<Path>,
    format: DistributionFormat,
) -> Result<PathBuf> {
    let sub_dir_name = format!("{}s", format.to_string().to_lowercase());
    let output_sub_dir = dist_dir.as_ref().join(sub_dir_name);

    if output_sub_dir.exists() {
        remove_dir_all(&output_sub_dir)
            .await
            .context("Failed to remove old output directory.")?;
    }

    create_dir_all(&output_sub_dir)
        .await
        .context("Failed to create output directory.")?;

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

    info!(
        "Successfully processed {}/{} {}s.",
        successes, total, dist_format
    );

    if errors.is_empty() {
        Ok(())
    } else {
        eprintln!(
            "Failed to process {}/{} {}:",
            errors.len(),
            total,
            dist_format
        );

        for (i, e) in errors.iter().enumerate() {
            eprintln!("  Failure {}: {}", i + 1, e);
        }

        Err(errors.remove(0))
    }
}

async fn distribute_as_txt(input: String, output_file: impl AsRef<Path>) -> Result<()> {
    fs::write(output_file, input).await?;

    Ok(())
}

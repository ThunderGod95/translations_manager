use crate::config::CONFIG;
use crate::pandoc::{PandocArgs, PandocMetadata};
use crate::util::{log_error, log_info, log_success, normalize_path, sort_indexed_results};
use anyhow::{Context, Result, anyhow};
use chrono::Utc;
use futures::future::join_all;
use futures::stream::FuturesOrdered;
use path_clean::PathClean;
use sanitize_filename::sanitize;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use strum::Display;
use tokio::fs::{create_dir_all, metadata, read_to_string, remove_dir_all};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

use futures::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct VolumeInfo {
    #[serde(rename = "filestart")]
    file_start: usize,
    #[serde(rename = "fileend")]
    file_end: usize,
    position: usize,
    title: String,
    series: String,
    author: String,
    translator: String,
    rights: String,
    image: String,
}

impl VolumeInfo {
    async fn load(from: impl AsRef<Path>) -> Result<Vec<Self>> {
        let content = read_to_string(from)
            .await
            .context("Failed to read volume separation file")?;

        let data =
            serde_json::from_str(&content).context("Failed to parse volume separation info.")?;

        Ok(data)
    }
}

#[derive(Debug, Clone, Copy, Display, PartialEq, Eq)]
pub enum DistributionFormat {
    EPUB,
    PDF,
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

    let volumes = VolumeInfo::load(assets_dir.join(&CONFIG.sep_info_file)).await?;

    log_info(format!(
        "Creating {}s... (Total: {})",
        dist_format,
        volumes.len()
    ));

    let futures = {
        let mut futures = Vec::with_capacity(volumes.len());

        for vol in &volumes {
            let fut = process_volume(
                vol,
                dist_format,
                translations_dir,
                assets_dir,
                &output_sub_dir,
            );
            futures.push(fut);
        }

        futures
    };

    let results = join_all(futures).await;

    log_errors(dist_format, results)
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

    log_success(format!(
        "Successfully processed {}/{} {}s.",
        successes, total, dist_format
    ));

    if errors.is_empty() {
        Ok(())
    } else {
        log_error(format!(
            "Failed to process {}/{} {}:",
            errors.len(),
            total,
            dist_format
        ));

        for (i, e) in errors.iter().enumerate() {
            log_error(format!("  Failure {}: {}", i + 1, e));
        }

        Err(errors.remove(0))
    }
}

async fn process_volume(
    vol: &VolumeInfo,
    dist_format: DistributionFormat,
    translations_dir: &Path,
    assets_dir: &Path,
    output_dir: &Path,
) -> Result<()> {
    log_info(format!("Creating {} for {}", dist_format, &vol.title));

    let output_file_name = sanitize(&vol.title);
    let output_file_path = output_dir.join(format!(
        "{}.{}",
        output_file_name,
        dist_format.to_string().to_lowercase()
    ));

    let metadata = build_pandoc_metadata(&vol, &assets_dir)?;
    let args = build_pandoc_args(
        dist_format,
        &metadata,
        &translations_dir,
        &assets_dir,
        &output_file_path,
    );
    let (total_files, input) = build_input(
        &translations_dir,
        &vol,
        dist_format,
        metadata.get_cover_image(),
    )
    .await?;

    log_success(format!(
        "Successfully read and validated {} chapter files ({}...{}) for {}",
        total_files, vol.file_start, vol.file_end, &vol.title
    ));

    run_pandoc(args, input).await
}

async fn run_pandoc(pandoc_args: PandocArgs, input: String) -> Result<()> {
    let mut cmd = Command::new("pandoc");
    cmd.args(pandoc_args.get())
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    let mut child = cmd.spawn().context(
        "Failed to run 'pandoc'. Check whether you have installed 'pandoc' and have it in PATH.",
    )?;

    let mut stdin = child
        .stdin
        .take()
        .context("Unexpected error. Failed to get child stdin.")?;

    stdin
        .write_all(input.as_bytes())
        .await
        .context("Failed to pass chapters' content to pandoc.")?;

    drop(stdin);

    let status = child
        .wait()
        .await
        .context("Failed to wait on pandoc process")?;

    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("Pandoc exited with status {}", status))
    }
}

fn build_pandoc_metadata(
    vol_info: &VolumeInfo,
    assets_dir: impl AsRef<Path>,
) -> Result<PandocMetadata> {
    let cover_image_path = assets_dir.as_ref().join(&vol_info.image).clean();

    let current_date = Utc::now().format("%Y-%m-%d").to_string();

    let mut pandoc_metadata = PandocMetadata::default();

    if cover_image_path.exists() {
        pandoc_metadata.set_cover_image(cover_image_path);
    }

    pandoc_metadata
        .add("title", &vol_info.title)
        .add("creator", &vol_info.author)
        .add("translator", &vol_info.translator)
        .add("rights", &vol_info.rights)
        .add("date", &current_date)
        .add("lang", "en-US")
        .add("belongs-to-collection", &vol_info.series)
        .add("collection-type", "series")
        .add("group-position", &vol_info.position)
        .add("publisher", &vol_info.translator)
        .add("pdftitle", &vol_info.title)
        .add("pdfauthor", &vol_info.author);

    Ok(pandoc_metadata)
}

fn build_pandoc_args(
    dist_format: DistributionFormat,
    metadata: &PandocMetadata,
    translations_dir: impl AsRef<Path>,
    assets_dir: impl AsRef<Path>,
    output_path: impl AsRef<Path>,
) -> PandocArgs {
    let translations_dir = translations_dir.as_ref().display().to_string();
    let assets_dir = assets_dir.as_ref().display().to_string();
    let output_path = output_path.as_ref().display().to_string();

    let mut pandoc_args = PandocArgs::default();

    pandoc_args
        .push_arg("--from")
        .push_arg("markdown-yaml_metadata_block")
        .push_arg("--resource-path")
        .push_arg(normalize_path(translations_dir))
        .push_arg("--resource-path")
        .push_arg(normalize_path(assets_dir))
        .push_arg("-o")
        .push_arg(normalize_path(output_path))
        .push_arg("--toc")
        .push_arg("--top-level-division=chapter");

    pandoc_args.set_variable("documentclass", "book");

    if dist_format == DistributionFormat::PDF {
        pandoc_args.set_pdf_engine("xelatex");
        pandoc_args
            .set_variable("fontsize", "12pt")
            .set_variable("geometry", "margin=1.2in")
            .set_variable("mainfont", "Book Antiqua")
            .set_variable("classoption", "openany")
            .set_variable("linestretch", "1.25");
    }

    let normalized_cover_image = metadata.get_cover_image();

    if dist_format == DistributionFormat::EPUB && normalized_cover_image.is_some() {
        pandoc_args.push_arg(format!(
            "--epub-cover-image={}",
            normalized_cover_image.unwrap()
        ));
    }

    for arg in metadata.get_metadata_args() {
        pandoc_args.push_arg(arg);
    }

    pandoc_args
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

/// Collects and validates all required `.md` files in the input directory concurrently.
async fn collect_md_files(input_dir: &Path, vol_info: &VolumeInfo) -> Result<(Vec<PathBuf>, u64)> {
    let num_files = (vol_info.file_end.saturating_sub(vol_info.file_start) + 1) as usize;
    let mut tasks = Vec::with_capacity(num_files);

    for (i, num) in (vol_info.file_start..=vol_info.file_end).enumerate() {
        let file_path = input_dir.join(format!("{}.md", num));

        let task = async move {
            let md = metadata(&file_path)
                .await
                .with_context(|| format!("Failed to get metadata for: {}", file_path.display()))?;

            if !md.is_file() {
                return Err(anyhow!(
                    "File {}.md exists but is not a regular file (Volume {}).",
                    num,
                    vol_info.position
                ));
            }

            let payload = (file_path, md.len());
            Ok((i, payload))
        };
        tasks.push(task);
    }

    let results = join_all(tasks).await;

    let sorted_files = sort_indexed_results(results)?;

    let mut file_paths = Vec::with_capacity(num_files);
    let mut total_size: u64 = 0;

    for (file_path, file_size) in sorted_files {
        total_size = total_size
            .checked_add(file_size)
            .ok_or_else(|| anyhow!("Total size overflow while summing file sizes"))?;

        file_paths.push(file_path);
    }

    if file_paths.len() != num_files {
        return Err(anyhow!(
            "Expected {} files. Found {}.",
            num_files,
            file_paths.len()
        ));
    }

    Ok((file_paths, total_size))
}

/// Builds the cover prefix for PDF output if a cover image is provided.
fn build_cover_prefix(
    dist_format: DistributionFormat,
    cover_path: Option<&Path>,
) -> Result<(String, u64)> {
    let mut cover_prefix = String::new();
    let mut added_size = 0;

    if dist_format == DistributionFormat::PDF {
        if let Some(path) = cover_path {
            let prefix = format!("![Cover]({})\n\n\\newpage\n\n", path.display());
            added_size = prefix.len() as u64;
            cover_prefix = prefix;
        }
    }

    Ok((cover_prefix, added_size))
}

/// Orchestrates file collection, size estimation, and content assembly.
async fn build_input(
    input_dir: impl AsRef<Path>,
    vol_info: &VolumeInfo,
    dist_format: DistributionFormat,
    normalized_cover_path: Option<impl AsRef<Path>>,
) -> Result<(usize, String)> {
    let input_dir = input_dir.as_ref();
    let cover_path = normalized_cover_path.as_ref().map(|p| p.as_ref());

    let (file_paths, mut total_size) = collect_md_files(input_dir, vol_info).await?;

    if file_paths.is_empty() {
        return Err(anyhow!("No files to add in: {}", &vol_info.title));
    }

    // Add cover (PDF only)
    let (cover_prefix, cover_size) = build_cover_prefix(dist_format, cover_path)?;

    total_size = total_size
        .checked_add(cover_size)
        .ok_or_else(|| anyhow!("Total size overflow after adding cover"))?;

    // Account for separators between files
    if file_paths.len() > 1 {
        let separator_bytes = 2 * (file_paths.len() - 1); // "\n\n"
        total_size = total_size
            .checked_add(separator_bytes as u64)
            .ok_or_else(|| anyhow!("Total size overflow after adding separators"))?;
    }

    let final_capacity = usize::try_from(total_size)
        .context("Unexpected error: Total content size exceeds bounds. Likely a bug.")?;

    let mut final_content = String::with_capacity(final_capacity);

    final_content.push_str(&cover_prefix);

    let mut reads_futures = FuturesOrdered::new();

    for (i, file_path) in file_paths.iter().enumerate() {
        let task = async move {
            let content = read_to_string(&file_path)
                .await
                .with_context(|| format!("Failed to read file: {}", file_path.display()))?;

            let content = content
                .strip_prefix('\u{FEFF}')
                .unwrap_or(&content)
                .to_string();

            Ok((i, content))
        };

        reads_futures.push_back(task);
    }

    let results = reads_futures.collect::<Vec<_>>().await;

    let sorted_contents = sort_indexed_results(results).context("Failed to sort file contents")?;

    for (i, contents) in sorted_contents.iter().enumerate() {
        if i > 0 {
            final_content.push_str("\n\n");
        }
        final_content.push_str(contents);
    }

    Ok((sorted_contents.len(), final_content))
}

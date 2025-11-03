use crate::config::CONFIG;
use crate::pandoc::{PandocArgs, PandocMetadata};
use crate::util::{log_info, normalize_path};
use anyhow::{anyhow, Context, Result};
use chrono::Utc;
use futures::future::join_all;
use path_clean::PathClean;
use sanitize_filename::sanitize;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use strum::Display;
use tokio::fs::{create_dir_all, metadata, read_to_string, remove_dir_all};
use tokio::io::AsyncWriteExt;
use tokio::process::Command;

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
) -> Result<Vec<Result<()>>> {
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

    Ok(results)
}

async fn process_volume(
    vol: &VolumeInfo,
    dist_format: DistributionFormat,
    translations_dir: &Path,
    assets_dir: &Path,
    output_dir: &Path,
) -> Result<()> {
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
    let input = build_input(
        &translations_dir,
        &vol,
        dist_format,
        metadata.get_cover_image(),
    )
    .await?;

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
    let output_sub_dir = dist_dir.as_ref().join(format.to_string().to_lowercase());

    if output_sub_dir.exists() {
        remove_dir_all(&output_sub_dir).await?;
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

    for i in vol_info.file_start..=vol_info.file_end {
        let file_path = input_dir.join(format!("{}.md", i));

        let task = async move {
            let md = metadata(&file_path)
                .await // Use the async version
                .with_context(|| format!("Failed to get metadata for: {}", file_path.display()))?;

            if !md.is_file() {
                return Err(anyhow!(
                    "File {}.md exists but is not a regular file (Volume {}).",
                    i,
                    vol_info.position
                ));
            }

            Ok((file_path, md.len()))
        };
        tasks.push(task);
    }

    let results = join_all(tasks).await;

    let mut file_paths = Vec::with_capacity(num_files);
    let mut total_size: u64 = 0;

    for result in results {
        let (file_path, file_size) = result?;

        total_size = total_size
            .checked_add(file_size)
            .ok_or_else(|| anyhow!("Total size overflow while summing file sizes"))?;

        file_paths.push(file_path);
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
) -> Result<String> {
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

    let final_capacity =
        usize::try_from(total_size).context("Total content size exceeds usize capacity")?;

    let mut final_content = String::with_capacity(final_capacity);
    final_content.push_str(&cover_prefix);

    let mut read_tasks = Vec::with_capacity(file_paths.len());

    for file_path in file_paths {
        let task = async move {
            read_to_string(&file_path)
                .await
                .with_context(|| format!("Failed to read file: {}", file_path.display()))
        };
        read_tasks.push(task);
    }

    let content_results = join_all(read_tasks).await;

    for (index, result) in content_results.into_iter().enumerate() {
        let content = result?;

        if index > 0 {
            final_content.push_str("\n\n");
        }
        final_content.push_str(&content);
    }

    Ok(final_content)
}

use super::{DistributionFormat, VolumeInfo};
use anyhow::{Context, Result, anyhow};
use futures::{
    StreamExt, TryStreamExt,
    stream::{self, FuturesOrdered},
};
use std::path::{Path, PathBuf};
use tokio::fs::{metadata, read_to_string};

pub(super) async fn build_input(
    input_dir: impl AsRef<Path>,
    vol_info: &VolumeInfo,
    dist_format: DistributionFormat,
    normalized_cover_path: Option<&str>,
) -> Result<(usize, String)> {
    let input_dir = input_dir.as_ref();
    let cover_path = normalized_cover_path.map(Path::new);

    let (file_paths, mut total_size) =
        collect_md_files(input_dir, vol_info.file_start, vol_info.file_end).await?;

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

    let file_count = read_files(&file_paths, &mut final_content).await?;

    Ok((file_count, final_content))
}

async fn collect_md_files(
    input_dir: &Path,
    start: usize,
    end: usize,
) -> Result<(Vec<PathBuf>, u64)> {
    let mut results = stream::iter(start..=end)
        .map(|file_num| {
            let file_path = input_dir.join(format!("{}.md", file_num));

            async move {
                let md = metadata(&file_path).await.with_context(|| {
                    format!("Failed to get metadata for file: {}", file_path.display())
                })?;

                if !md.is_file() {
                    return Err(anyhow!(
                        "Path {} exists but is not a regular file.",
                        file_path.display()
                    ));
                }

                Ok((file_num, file_path, md.len()))
            }
        })
        .buffered(200) // Concurrently check up to 200 files
        .try_collect::<Vec<_>>()
        .await?;

    results.sort_by_key(|(file_num, _, _)| *file_num);

    let results_len = results.len();

    let (file_paths, total_size) = results.into_iter().try_fold(
        (Vec::with_capacity(results_len), 0u64),
        |mut acc, (_, path, size)| -> Result<_> {
            acc.0.push(path);
            acc.1 = acc
                .1
                .checked_add(size)
                .ok_or_else(|| anyhow!("Total size overflow while summing file sizes"))?;
            Ok(acc)
        },
    )?;

    Ok((file_paths, total_size))
}

async fn read_files(file_paths: &Vec<PathBuf>, buffer: &mut String) -> Result<usize> {
    let mut read_futures = FuturesOrdered::new();

    for file_path in file_paths.iter() {
        let task = async move {
            let content = read_to_string(file_path)
                .await
                .with_context(|| format!("Failed to read file: {}", file_path.display()))?;

            Ok::<_, anyhow::Error>(content)
        };
        read_futures.push_back(task);
    }

    let mut file_count = 0;
    while let Some(result) = read_futures.next().await {
        let content_str = result?;

        if file_count > 0 {
            buffer.push_str("\n\n"); // Add separator
        }

        // Strip BOM if present
        let content_slice = content_str.strip_prefix('\u{FEFF}').unwrap_or(&content_str);
        buffer.push_str(content_slice);
        file_count += 1;
    }

    Ok(file_count)
}

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

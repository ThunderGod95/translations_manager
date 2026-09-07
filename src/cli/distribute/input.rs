use crate::cli::clean;
use crate::cli::util::collect_numbered_file_paths;

use super::{DistributionFormat, VolumeInfo};

use anyhow::{Context, Result, anyhow};
use comrak::nodes::NodeValue;
use comrak::{Arena, Options, format_commonmark, parse_document};
use rayon::prelude::*;
use std::fs::read_to_string;
use std::path::{Path, PathBuf};

pub(super) fn build_input(
    input_dir: impl AsRef<Path>,
    vol_info: &VolumeInfo,
    dist_format: DistributionFormat,
    normalized_cover_path: Option<&str>,
) -> Result<String> {
    let input_dir = input_dir.as_ref();
    let cover_path = normalized_cover_path.map(Path::new);

    let (file_paths, mut total_size) = collect_numbered_file_paths(
        input_dir,
        Some("md"),
        Some(vol_info.file_start),
        Some(vol_info.file_end),
    )?;

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

    read_files(&file_paths, &mut final_content)?;

    Ok(final_content)
}

fn read_files(file_paths: &[PathBuf], buffer: &mut String) -> Result<()> {
    let mut indexed_results: Vec<(usize, String)> = file_paths
        .par_iter()
        .enumerate()
        .map(|(index, file_path)| {
            let content = read_to_string(file_path)
                .with_context(|| format!("Failed to read file: {}", file_path.display()));

            content.map(|content| (index, content))
        })
        .collect::<Result<Vec<_>>>()?;

    indexed_results.sort_by_key(|(index, _)| *index);

    for (position, (_, content)) in indexed_results.into_iter().enumerate() {
        if position > 0 {
            buffer.push_str("\n\n");
        }

        let content = content.strip_prefix('\u{FEFF}').unwrap_or(&content);

        let content = clean::sanitize_chapter(content);

        let content = scope_footnotes(&content, position + 1).with_context(|| {
            format!(
                "Failed to process footnotes in {}",
                file_paths[position].display()
            )
        })?;

        buffer.push_str(&content);
    }

    Ok(())
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

fn scope_footnotes(content: &str, chapter_scope: usize) -> Result<String> {
    let arena = Arena::new();

    let mut options = Options::default();
    options.extension.footnotes = true;

    let root = parse_document(&arena, content, &options);

    for node in root.descendants() {
        let mut data = node.data.borrow_mut();

        match &mut data.value {
            NodeValue::FootnoteDefinition(definition) => {
                definition.name = format!("chapter-{chapter_scope}-{}", definition.name);
            }

            NodeValue::FootnoteReference(reference) => {
                reference.name = format!("chapter-{chapter_scope}-{}", reference.name);
            }

            _ => {}
        }
    }

    let mut output = String::new();
    format_commonmark(root, &options, &mut output)?;

    Ok(output)
}

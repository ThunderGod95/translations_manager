use std::{
    fmt::Write,
    fs::{File, create_dir_all, metadata, read_dir, read_to_string},
    io::{BufReader, ErrorKind, Read},
    path::{Path, PathBuf},
};

use anyhow::{Context, Result, anyhow};
use chrono::Utc;
use rayon::prelude::*;
use sanitize_filename::Options;
use serde::{Deserialize, Serialize};
use strum::Display;

use crate::{
    config::CONFIG,
    util::{clear_dir_contents, get_sorted_md_file_paths, log_info},
};

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
    image: PathBuf,
}

impl VolumeInfo {
    fn load(from: impl AsRef<Path>) -> Result<Vec<Self>> {
        let file = File::open(from)?;
        let reader = BufReader::new(file);
        let data =
            serde_json::from_reader(reader).context("Failed to load volume separation info.")?;
        Ok(data)
    }
}

#[derive(Debug, Clone, Copy, Display, PartialEq, Eq)]
pub enum DistributionFormat {
    EPUB,
    PDF,
}

// pub fn distribute(
//     dist_format: DistributionFormat,
//     translations_dir: impl AsRef<Path>,
//     assets_dir: impl AsRef<Path>,
//     dist_dir: impl AsRef<Path>,
// ) -> Result<()> {
//     let translations_dir = translations_dir.as_ref();
//     let assets_dir = assets_dir.as_ref();

//     let volumes = VolumeInfo::load(assets_dir.join(&CONFIG.sep_info_file))?;
//     let num_volumes = volumes.len();

//     log_info(format!("Creating {} {}s...", num_volumes, dist_format));

//     let output_dir = dist_dir
//         .as_ref()
//         .join(dist_format.to_string().to_lowercase());

//     fs::create_dir_all(&output_dir)?;

//     let file_paths = get_sorted_md_file_paths(translations_dir)?;

//     let pb = ProgressBar::new(num_volumes as u64);
//     pb.set_style(
//         ProgressStyle::default_bar()
//             .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} ({percent}%) - {msg}")
//             .expect("Invalid progress bar template")
//             .progress_chars("#>-"),
//     );

//     let results: Result<Vec<()>> = volumes
//         .par_iter()
//         .map(|volume| {
//             pb.set_message(volume.title.clone());

//             let file_name = get_sanitized_name(&volume.title, dist_format);
//             let (start_index, end_index) =
//                 normalize_indexes(&file_paths, volume.file_start, volume.file_end)?;

//             let mut pandoc = pandoc(dist_format, &assets_dir)?;

//             pandoc.set_variable("title", &volume.title);
//             pandoc.set_variable("author", &volume.author);
//             pandoc.set_variable("translator", &volume.translator);
//             pandoc.set_variable("rights", &volume.rights);
//             pandoc.set_variable("series", &volume.series);
//             pandoc.set_variable("group-position", &volume.position.to_string());
//             pandoc.set_variable("publisher", "ThunderGod95");
//             pandoc.set_variable(
//                 "identifier",
//                 "https://github.com/ThunderGod95/TheMirrorLegacy",
//             );
//             let now = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
//             pandoc.set_variable("date", &now);

//             let image_path = assets_dir.join(&volume.image);

//             if image_path.exists() {
//                 pandoc.set_variable("cover-image", &image_path.to_string_lossy());
//             } else {
//                 return Err(anyhow!(
//                     "Cover image '{}' not found in assets for volume: {}",
//                     volume.image.display(),
//                     volume.title
//                 ));
//             }

//             let output_file_path = output_dir.join(&file_name);
//             pandoc.set_output(OutputKind::File(output_file_path));

//             let file_to_add = &file_paths[start_index..=end_index];

//             for file in file_to_add {
//                 pandoc.add_input(file);
//             }

//             pandoc.execute()?;

//             pb.inc(1);

//             Ok(())
//         })
//         .collect();

//     results?;

//     pb.finish_with_message(format!(
//         "Successfully created {} {}s",
//         num_volumes, dist_format
//     ));

//     Ok(())
// }

// fn pandoc(dist_format: DistributionFormat, assets_dir: impl AsRef<Path>) -> Result<Pandoc> {
//     let assets_dir = assets_dir.as_ref();
//     let mut pandoc = Pandoc::new();

//     pandoc.set_doc_class(DocumentClass::Book);
//     pandoc.set_show_cmdline(true);
//     pandoc.set_input_format(InputFormat::MarkdownGithub, vec![]);
//     pandoc.set_variable("lang", "en");

//     let css_file = assets_dir.join("epub.css");
//     let font_regular = assets_dir.join("fonts/BKANT.TTF");
//     let font_bold = assets_dir.join("fonts/ANTQUAB.TTF");

//     match dist_format {
//         DistributionFormat::EPUB => {
//             pandoc.set_output_format(OutputFormat::Epub3);

//             // Embed the font files into the EPUB
//             if font_regular.exists() {
//                 pandoc.add_option(PandocOption::EpubEmbedFont(font_regular));
//             }
//             if font_bold.exists() {
//                 pandoc.add_option(PandocOption::EpubEmbedFont(font_bold));
//             }

//             if css_file.exists() {
//                 let css_file_string = css_file.to_str().unwrap().to_owned();
//                 pandoc.add_option(PandocOption::Css(css_file_string));
//             }
//         }
//         DistributionFormat::PDF => {
//             pandoc.set_output_format(OutputFormat::Pdf);
//             pandoc.add_option(PandocOption::PdfEngine("xelatex".into()));

//             pandoc.set_variable("classoption", "openany");
//             pandoc.set_variable("mainfont", "Book Antiqua");
//             pandoc.set_variable("fontsize", "12pt");
//             pandoc.set_variable("geometry", "margin=1.5in");
//             pandoc.set_variable("linestretch", "1.25");

//             let tweak_chapter_space =
//                 "\\usepackage{titlesec}\n\\titlespacing*{\\chapter}{0pt}{0pt}{20pt}";
//             pandoc.set_variable("header-includes", tweak_chapter_space);
//         }
//     }

//     Ok(pandoc)
// }

// fn get_sanitized_name(title: &str, dist_format: DistributionFormat) -> String {
//     let sanitization_options = Options {
//         replacement: "-",
//         ..Default::default()
//     };

//     sanitize_filename::sanitize_with_options(
//         format!("{}.{}", title, dist_format.to_string().to_lowercase()),
//         sanitization_options,
//     )
// }

// fn normalize_indexes(
//     file_paths: &Vec<PathBuf>,
//     start: usize,
//     end: usize,
// ) -> Result<(usize, usize)> {
//     let start_stem = start.to_string();
//     let end_stem = end.to_string();

//     let start_index = file_paths
//         .iter()
//         .position(|path| {
//             path.file_stem()
//                 .and_then(|s| s.to_str())
//                 .map(|stem_str| stem_str.to_string())
//                 .map_or(false, |stem| stem == start_stem)
//         })
//         .ok_or_else(|| {
//             anyhow!(
//                 "Start file stem '{}' not found in translations_dir",
//                 start_stem
//             )
//         })?;

//     let end_index_relative = file_paths[start_index..]
//         .iter()
//         .position(|path| {
//             path.file_stem()
//                 .and_then(|s| s.to_str())
//                 .map(|stem_str| stem_str.to_string())
//                 .map_or(false, |stem| stem == end_stem)
//         })
//         .ok_or_else(|| {
//             anyhow!(
//                 "End file stem '{}' not found *after* start file '{}'",
//                 end_stem,
//                 start_stem
//             )
//         })?;

//     let end_index = start_index + end_index_relative;

//     Ok((start_index, end_index))
// }

struct PandocMetadata {
    metadata_args: Vec<String>,
    normalized_cover_path: Option<String>,
}

fn prepare_output_directory(
    dist_dir: impl AsRef<Path>,
    format: DistributionFormat,
) -> Result<PathBuf> {
    let output_sub_dir = dist_dir.as_ref().join(format.to_string().to_lowercase());

    if !output_sub_dir.exists() {
        create_dir_all(&output_sub_dir)?;
    } else {
        clear_dir_contents(&output_sub_dir)?;
    }

    Ok(output_sub_dir)
}

/// Collects and validates all required `.md` files in the input directory.
fn collect_md_files(input_dir: &Path, vol_info: &VolumeInfo) -> Result<(Vec<PathBuf>, u64)> {
    let num_files = (vol_info.file_end.saturating_sub(vol_info.file_start) + 1) as usize;
    let mut file_paths = Vec::with_capacity(num_files);
    let mut total_size: u64 = 0;

    for i in vol_info.file_start..=vol_info.file_end {
        let file_path = input_dir.join(format!("{}.md", i));

        let md = metadata(&file_path)
            .with_context(|| format!("Failed to get metadata for: {}", file_path.display()))?;

        if !md.is_file() {
            return Err(anyhow!(
                "File {}.md exists but is not a regular file (Volume {}).",
                i,
                vol_info.position
            ));
        }

        total_size = total_size
            .checked_add(md.len())
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
            let prefix = format!("![Cover]({})\n\n\\newpage\n", path.display());
            added_size = prefix.len() as u64;
            cover_prefix = prefix;
        }
    }

    Ok((cover_prefix, added_size))
}

/// Orchestrates file collection, size estimation, and content assembly.
fn build_input(
    input_dir: impl AsRef<Path>,
    vol_info: &VolumeInfo,
    dist_format: DistributionFormat,
    normalized_cover_path: Option<impl AsRef<Path>>,
) -> Result<String> {
    let input_dir = input_dir.as_ref();
    let cover_path = normalized_cover_path.as_ref().map(|p| p.as_ref());

    let (file_paths, mut total_size) = collect_md_files(input_dir, vol_info)?;

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

    let mut read_buffer = String::new();

    for (index, file_path) in file_paths.iter().enumerate() {
        if index > 0 {
            final_content.push_str("\n\n");
        }

        let mut file = File::open(file_path)
            .with_context(|| format!("Failed to open file: {}", file_path.display()))?;

        read_buffer.clear();

        file.read_to_string(&mut read_buffer)
            .with_context(|| format!("Failed to read file: {}", file_path.display()))?;

        final_content.push_str(&read_buffer);
    }

    Ok(final_content)
}

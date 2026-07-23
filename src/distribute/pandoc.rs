use super::{DistributionFormat, VolumeInfo};
use crate::util::{get_current_date, normalize_path};
use anyhow::{anyhow, Context, Result};
use console::style;
use path_clean::PathClean;
use std::fmt::Display;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Default)]
pub struct PandocArgs(Vec<String>);

impl PandocArgs {
    pub fn push_arg(&mut self, arg: impl Display) -> &mut Self {
        self.0.push(format!("{}", arg));
        self
    }

    pub fn include_in_header(&mut self, header: impl AsRef<Path>) -> &mut Self {
        self.0.push("--include-in-header".to_string());
        self.0.push(header.as_ref().display().to_string());
        self
    }

    pub fn embed_font(&mut self, font: impl AsRef<Path>) -> &mut Self {
        self.0
            .push(format!("--epub-embed-font={}", font.as_ref().display()));
        self
    }

    pub fn embed_css(&mut self, css: impl AsRef<Path>) -> &mut Self {
        self.0.push(format!("--css={}", css.as_ref().display()));
        self
    }

    pub fn set_pdf_engine(&mut self, engine: impl Display) -> &mut Self {
        self.0.push(format!("--pdf-engine={}", engine));
        self
    }

    pub fn set_variable<K: Display, V: Display>(&mut self, key: K, value: V) -> &mut Self {
        self.0.push(format!("--variable={}:{}", key, value));
        self
    }

    pub fn set_reference_doc(&mut self, doc: impl AsRef<Path>) -> &mut Self {
        self.0
            .push(format!("--reference-doc={}", doc.as_ref().display()));
        self
    }

    pub fn get(&self) -> &Vec<String> {
        &self.0
    }
}

impl Display for PandocArgs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.join(" "))
    }
}

#[derive(Debug, Default)]
pub struct PandocMetadata {
    metadata_args: Vec<String>,
    normalized_cover_path: Option<String>,
}

impl PandocMetadata {
    pub fn add<K: Display, V: Display>(&mut self, key: K, value: V) -> &mut Self {
        self.metadata_args
            .push(format!("--metadata={}:{}", key, value));
        self
    }

    pub fn set_cover_image(&mut self, path: impl AsRef<Path>) {
        let clean_path_string = path.as_ref().display().to_string().replace("\\", "/");
        self.normalized_cover_path = Some(clean_path_string);
    }

    pub fn get_cover_image(&self) -> Option<String> {
        self.normalized_cover_path.clone()
    }

    pub fn get_metadata_args(&self) -> &Vec<String> {
        &self.metadata_args
    }
}

pub(super) fn run(pandoc_args: PandocArgs, input: String) -> Result<()> {
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
        .context("Failed to pass chapters' content to pandoc.")?;

    // Close stdin to signal EOF to pandoc
    drop(stdin);

    let status = child.wait().context("Failed to wait on pandoc process")?;

    if status.success() {
        Ok(())
    } else {
        Err(anyhow!("Pandoc exited with status {}", status))
    }
}

pub(super) fn build_metadata(
    vol_info: &VolumeInfo,
    assets_dir: impl AsRef<Path>,
) -> Result<PandocMetadata> {
    let cover_image_path = assets_dir.as_ref().join(&vol_info.image).clean();
    let current_date = get_current_date()?;
    let mut pandoc_metadata = PandocMetadata::default();

    if cover_image_path.exists() {
        pandoc_metadata.set_cover_image(cover_image_path);
    }

    pandoc_metadata
        .add("title", &vol_info.title)
        .add("author", &vol_info.author)
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

pub(super) fn build_args(
    dist_format: DistributionFormat,
    metadata: &PandocMetadata,
    translations_dir: impl AsRef<Path>,
    assets_dir: impl AsRef<Path>,
    output_path: impl AsRef<Path>,
) -> Result<PandocArgs> {
    let translations_dir = translations_dir.as_ref();
    let assets_dir = assets_dir.as_ref();
    let output_path = output_path.as_ref();

    let mut pandoc_args = PandocArgs::default();

    pandoc_args
        .push_arg("--from")
        .push_arg("markdown-yaml_metadata_block-multiline_tables")
        .push_arg("--resource-path")
        .push_arg(normalize_path(translations_dir.display().to_string()))
        .push_arg("--resource-path")
        .push_arg(normalize_path(assets_dir.display().to_string()))
        .push_arg("-o")
        .push_arg(normalize_path(output_path.display().to_string()))
        .push_arg("--toc")
        .push_arg("--top-level-division=chapter");

    pandoc_args.set_variable("documentclass", "scrbook");

    match dist_format {
        DistributionFormat::PDF => {
            apply_pdf_args(&mut pandoc_args, assets_dir)?;
        }
        DistributionFormat::EPUB => {
            apply_epub_args(&mut pandoc_args, metadata, assets_dir)?;
        }
        DistributionFormat::DOCX => {
            apply_docx_args(&mut pandoc_args, assets_dir)?;
        }
        _ => {}
    }

    for arg in metadata.get_metadata_args() {
        pandoc_args.push_arg(arg);
    }

    Ok(pandoc_args)
}

fn apply_pdf_args(pandoc_args: &mut PandocArgs, assets_dir: &Path) -> Result<()> {
    pandoc_args.set_pdf_engine("xelatex");

    let pdf_style_path = assets_dir.join("dist").join("style.tex");

    if fs::metadata(&pdf_style_path).is_ok() {
        println!("Found PDF styles. Applying them...");

        pandoc_args.include_in_header(pdf_style_path);
    } else {
        println!(
            "{}",
            style(format!(
                "[WARN] PDF styles not found. Applying default styles..."
            ))
            .yellow()
        );

        pandoc_args
            .set_variable("linestretch", "1.25")
            .set_variable("geometry", "margin=1.2in")
            .set_variable("mainfont", "\"Book Antiqua\"");
    }

    pandoc_args
        .set_variable("fontsize", "12pt")
        .set_variable("classoption", "openany");

    Ok(())
}

fn apply_epub_args(
    pandoc_args: &mut PandocArgs,
    metadata: &PandocMetadata,
    assets_dir: &Path,
) -> Result<()> {
    let epub_dist_info = assets_dir.join("dist");

    if epub_dist_info.is_dir() {
        let mut dirs_to_visit = vec![epub_dist_info];

        while let Some(current_dir) = dirs_to_visit.pop() {
            if let Ok(entries) = fs::read_dir(current_dir) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let entry_path = entry.path();

                    if entry_path.is_dir() {
                        dirs_to_visit.push(entry_path);
                        continue;
                    }

                    if let Some(ex_str) = entry_path.extension().and_then(|s| s.to_str()) {
                        match ex_str {
                            "css" => {
                                pandoc_args.embed_css(entry_path);
                            }
                            "ttf" => {
                                pandoc_args.embed_font(entry_path);
                            }
                            _ => {}
                        }
                    }
                }
            }
        }
    }

    if let Some(cover_image) = metadata.get_cover_image() {
        pandoc_args.push_arg(format!("--epub-cover-image={}", cover_image));
    }

    Ok(())
}

fn apply_docx_args(pandoc_args: &mut PandocArgs, assets_dir: &Path) -> Result<()> {
    let reference_doc_path = assets_dir.join("dist").join("reference.docx");

    if fs::metadata(&reference_doc_path).is_ok() {
        println!("Found DOCX reference document. Applying it...");
        pandoc_args.set_reference_doc(reference_doc_path);
    } else {
        println!(
            "{}",
            style("[WARN] Reference DOCX not found. Using Pandoc defaults.").yellow()
        );
    }

    Ok(())
}

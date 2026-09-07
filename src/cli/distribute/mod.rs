mod input;
mod pandoc;

use std::collections::{HashMap, HashSet};
use std::fs::{self, create_dir_all};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result, bail};
use clap::ValueEnum;
use console::style;
use indicatif::{ProgressBar, ProgressStyle};
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use strum::{Display, VariantArray};
use threadpool::ThreadPool;

use crate::cli::config::{CONFIG, ProjectPaths};

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

    pub fn only_required(from: &Vec<VolumeInfo>, required_volumes: &[usize]) -> Result<Vec<Self>> {
        let volumes_to_process: Vec<_> = if required_volumes.is_empty() {
            from.clone()
        } else {
            let available_positions: HashSet<usize> = from.iter().map(|v| v.position).collect();

            let mut missing_volumes: Vec<usize> = required_volumes
                .iter()
                .filter(|&req| !available_positions.contains(req))
                .copied()
                .collect();

            let available_positions: Vec<_> = available_positions.iter().sorted().collect();
            missing_volumes.sort();

            if !missing_volumes.is_empty() {
                bail!(
                    "The following requested volumes were not found in: {:?}.\nAvailable volumes are: {:?}",
                    missing_volumes,
                    available_positions
                );
            }

            from.clone()
                .into_iter()
                .filter(|vol| required_volumes.contains(&vol.position))
                .collect()
        };

        Ok(volumes_to_process)
    }
}

#[derive(
    Debug, Hash, Clone, Copy, Display, PartialEq, Eq, PartialOrd, Ord, VariantArray, ValueEnum,
)]
/// Represents the supported file formats for content distribution.
pub enum DistributionFormat {
    /// Electronic Publication format (standard eBooks).
    EPUB,
    /// Portable Document Format.
    PDF,
    /// Plain text format.
    TXT,
    /// MS Word format.
    DOCX,
}

/// Manages the concurrent processing and distribution of project files.
pub struct Distributor {
    vols_info: Vec<VolumeInfo>,
    paths: ProjectPaths,
    pool: ThreadPool,
    results: Arc<Mutex<HashMap<DistributionFormat, Vec<Result<()>>>>>,
    progress: ProgressBar,
}

impl Distributor {
    /// Creates a new `Distributor` instance.
    ///
    /// This initializes a thread pool with a size equal to the number of
    /// available logical CPUs on the current machine.
    ///
    /// # Returns
    ///
    /// A new instance of `Distributor` ready to accept tasks.
    pub fn new(project_name: &str) -> Result<Self> {
        let paths = ProjectPaths::new(&project_name);

        let config = CONFIG.read().expect("Config lock poisoned");
        let info_path = paths.assets_folder.join(&config.sep_info_file);

        let pb = ProgressBar::new(0);
        pb.set_style(
            ProgressStyle::with_template(
                "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}",
            )
            .unwrap()
            .progress_chars("#>-"),
        );
        pb.enable_steady_tick(Duration::from_millis(50));

        Ok(Self {
            vols_info: VolumeInfo::load(info_path)?,
            paths,
            pool: ThreadPool::new(num_cpus::get()),
            results: Arc::new(Mutex::new(HashMap::new())),
            progress: pb,
        })
    }

    /// Queues a complete project for distribution in the specified format.
    ///
    /// This method submits the task to the internal thread pool for asynchronous processing.
    ///
    /// # Arguments
    ///
    /// * `dist_format` - The target [`DistributionFormat`] (e.g., PDF, EPUB).
    /// * `project` - A string slice representing the project identifier or name.
    /// * `volumes` - The volumes to process. If empty, all volumes are processed.
    pub fn add(&self, dist_format: DistributionFormat, required_volumes: &[usize]) -> Result<()> {
        let output_sub_dir = prepare_output_directory(&self.paths.dist_folder, dist_format)?;

        let volumes = VolumeInfo::only_required(&self.vols_info, &required_volumes)?;

        self.progress.inc_length(volumes.len() as u64);
        self.progress.set_message(format!("Processing..."));

        for vol in volumes {
            let results = Arc::clone(&self.results);
            let translations_dir = self.paths.translations_folder.clone();
            let assets_dir = self.paths.assets_folder.clone();
            let output_sub_dir = output_sub_dir.clone();
            let pb = self.progress.clone();

            self.pool.execute(move || {
                let res = process_volume(
                    &vol,
                    dist_format,
                    &translations_dir,
                    &assets_dir,
                    &output_sub_dir,
                    &pb,
                );

                pb.inc(1);

                let mut results_guard = results.lock().unwrap();
                results_guard.entry(dist_format).or_default().push(res);
            });
        }

        Ok(())
    }

    /// Wait for all tasks to finish and returns errors if any.
    ///
    /// `wait` will return immediately if no tasks are being executed.
    pub fn wait(&self) -> Result<()> {
        self.pool.join();

        self.progress.finish_with_message("All tasks completed.");

        let mut results_guard = self.results.lock().unwrap_or_else(|e| {
            eprintln!(
                "{}",
                style("[WARN] One or more processing threads crashed. Results may be incomplete.")
                    .yellow()
            );
            e.into_inner()
        });

        let results = std::mem::take(&mut *results_guard);

        log_errors(results)
    }
}

fn process_volume(
    vol: &VolumeInfo,
    dist_format: DistributionFormat,
    translations_dir: &Path,
    assets_dir: &Path,
    output_dir: &Path,
    pb: &ProgressBar,
) -> Result<()> {
    pb.set_message(format!("{} -> {}", dist_format, &vol.title));

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

    if !output_sub_dir.exists() {
        create_dir_all(&output_sub_dir).with_context(|| {
            format!(
                "Failed to create the output directory: '{}'.\n  \
             Please check if you have write permissions for this location.",
                output_sub_dir.display()
            )
        })?;
    }

    Ok(output_sub_dir)
}

fn log_errors(results_map: HashMap<DistributionFormat, Vec<Result<()>>>) -> Result<()> {
    let mut final_result = Ok(());

    for (dist_format, results) in results_map {
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

        if !errors.is_empty() {
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

            if final_result.is_ok() {
                final_result = Err(errors.remove(0));
            }
        }
    }

    final_result
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

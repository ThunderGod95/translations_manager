use anyhow::{Context, Result};
use console::style;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::sync::LazyLock;

use crate::util::get_config_file_path;

pub static PROJECT_PATH_QUALIFIERS: [&str; 3] = ["com", "tg", "tscripts"];

pub static CONFIG: LazyLock<AppConfig> =
    LazyLock::new(|| load_config().expect("Failed to load configuration"));

#[derive(Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub translations_folder: PathBuf,
    pub assets_folder: PathBuf,
    pub dist_folder: PathBuf,
    pub raws_folder: PathBuf,
    pub glossary_file: PathBuf,
    pub chapter_file: PathBuf,
    pub translation_prompt_file: PathBuf,
    pub sep_info_file: PathBuf,
    pub cache_file: PathBuf,
    pub find_history_config_file: PathBuf,
    pub fuzzy_search_threshold: u32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            glossary_file: "glossary.json".into(),
            chapter_file: "cr_ch.txt".into(),
            translation_prompt_file: "translation_prompt.md".into(),
            sep_info_file: "sep.json".into(),
            cache_file: ".runner-cache.json".into(),
            find_history_config_file: ".find_history.txt".into(),
            fuzzy_search_threshold: 2,
            translations_folder: "translations".into(),
            assets_folder: "assets".into(),
            dist_folder: "dist".into(),
            raws_folder: "raws".into(),
        }
    }
}

fn load_config() -> Result<AppConfig> {
    let config_path = get_config_file_path()?;

    match fs::read_to_string(&config_path) {
        Ok(config_str) => {
            let config: AppConfig = toml::from_str(&config_str)
                .context(format!("Failed to parse config file at {:?}", config_path))?;

            let updated_config_str = toml::to_string_pretty(&config)?;

            if config_str != updated_config_str {
                println!(
                    "{}",
                    style(format!(
                        "[WARN] Config file at {:?} is outdated. Adding missing default values.",
                        config_path
                    ))
                    .yellow()
                );

                fs::write(&config_path, updated_config_str)
                    .context("Failed to write updated config file")?;
            }

            Ok(config)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            println!(
                "{}",
                style(format!(
                    "[WARN] Config file not found, creating default at: {:?}",
                    config_path
                ))
                .yellow()
            );

            let config = AppConfig::default();
            let config_str = toml::to_string_pretty(&config)?;

            if let Some(parent_dir) = config_path.parent() {
                if !parent_dir.exists() {
                    fs::create_dir_all(parent_dir)?;
                }
            }

            fs::write(config_path, config_str)?;

            Ok(config)
        }
        Err(e) => Err(e.into()),
    }
}

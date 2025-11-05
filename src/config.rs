use std::fs;

use anyhow::Result;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

use crate::util::get_config_file_path;

pub static PROJECT_PATH_QUALIFIERS: [&str; 3] = ["com", "tg", "tscripts"];

pub static CONFIG: Lazy<AppConfig> =
    Lazy::new(|| load_config().expect("Failed to load configuration"));

#[derive(Debug, Serialize, Deserialize)]
pub struct AppConfig {
    pub translations_folder: String,
    pub assets_folder: String,
    pub dist_folder: String,
    pub glossary_file: String,
    pub chapter_file: String,
    pub translation_prompt_file: String,
    pub sep_info_file: String,
    pub cache_file: String,
    pub find_history_config_file: String,
    pub fuzzy_search_threshold: i32,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            glossary_file: "glossary.json".to_string(),
            chapter_file: "cr_ch.txt".to_string(),
            translation_prompt_file: "translation_prompt.md".to_string(),
            sep_info_file: "sep.json".to_string(),
            cache_file: ".runner-cache.json".to_string(),
            find_history_config_file: ".find_history.txt".to_string(),
            fuzzy_search_threshold: 65,
            translations_folder: "translations".to_string(),
            assets_folder: "assets".to_string(),
            dist_folder: "dist".to_string(),
        }
    }
}

pub fn load_config() -> Result<AppConfig> {
    let config_path = get_config_file_path()?;

    if config_path.exists() {
        let config_str = fs::read_to_string(config_path)?;
        let config: AppConfig = toml::from_str(&config_str)?;
        Ok(config)
    } else {
        let config = AppConfig::default();
        let config_str = toml::to_string_pretty(&config)?;
        fs::write(config_path, config_str)?;
        Ok(config)
    }
}

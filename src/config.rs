use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::fs;
use tokio::sync::OnceCell;

use crate::util::get_config_file_path;

pub static PROJECT_PATH_QUALIFIERS: [&str; 3] = ["com", "tg", "tscripts"];

static CONFIG: OnceCell<AppConfig> = OnceCell::const_new();

/// Asynchronously gets the global application configuration.
pub async fn get_config() -> &'static AppConfig {
    CONFIG
        .get_or_init(|| async { load_config().await.expect("Failed to load configuration") })
        .await
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AppConfig {
    pub translations_folder: String,
    pub assets_folder: String,
    pub dist_folder: String,
    pub raws_folder: String,
    pub glossary_file: String,
    pub chapter_file: String,
    pub translation_prompt_file: String,
    pub sep_info_file: String,
    pub cache_file: String,
    pub find_history_config_file: String,
    pub fuzzy_search_threshold: u32,
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
            fuzzy_search_threshold: 2,
            translations_folder: "translations".to_string(),
            assets_folder: "assets".to_string(),
            dist_folder: "dist".to_string(),
            raws_folder: "raws".to_string(),
        }
    }
}

pub async fn load_config() -> Result<AppConfig> {
    let config_path = get_config_file_path().await?;

    match fs::read_to_string(&config_path).await {
        Ok(config_str) => {
            let config: AppConfig = toml::from_str(&config_str)?;
            Ok(config)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            println!(
                "Config file not found, creating default at: {:?}",
                config_path
            );
            let config = AppConfig::default();
            let config_str = toml::to_string_pretty(&config)?;
            fs::write(config_path, config_str).await?;
            Ok(config)
        }
        Err(e) => Err(e.into()),
    }
}

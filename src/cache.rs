use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tokio::fs;

use crate::util::get_cache_path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cache {
    #[serde(rename = "lastProject")]
    pub last_project: String,
}

pub async fn read_cache() -> Result<Option<Cache>> {
    let cache_path = get_cache_path().await?;
    if !cache_path.exists() {
        return Ok(None);
    }

    let cache_string = fs::read_to_string(&cache_path)
        .await
        .with_context(|| format!("Failed to read cache file at {}", cache_path.display()))?;

    let cache: Cache = serde_json::from_str(&cache_string)
        .with_context(|| format!("Failed to parse cache file at {}", cache_path.display()))?;

    Ok(Some(cache))
}

pub async fn write_cache(cache: Cache) -> Result<()> {
    let cache_path = get_cache_path().await?;
    let j = serde_json::to_string_pretty(&cache).context("Failed to serialize cache")?;

    fs::write(&cache_path, j)
        .await
        .with_context(|| format!("Failed to write cache file to {}", cache_path.display()))?;

    Ok(())
}

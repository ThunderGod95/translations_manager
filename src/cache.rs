use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;

use crate::util::get_cache_path;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Cache {
    #[serde(rename = "lastProject")]
    pub last_project: String,
    #[serde(rename = "hasRunBefore")]
    pub has_run_before: bool,
}

pub fn read_cache() -> Result<Option<Cache>> {
    let cache_path = get_cache_path()?;
    if !cache_path.exists() {
        return Ok(None);
    }

    let mut cache_data = fs::read(&cache_path)
        .with_context(|| format!("Failed to read cache file at {}", cache_path.display()))?;

    let cache: Cache = simd_json::from_slice(&mut cache_data)
        .with_context(|| format!("Failed to parse cache file at {}", cache_path.display()))?;

    Ok(Some(cache))
}

pub fn write_cache(cache: Cache) -> Result<()> {
    let cache_path = get_cache_path()?;
    let j = simd_json::to_string_pretty(&cache).context("Failed to serialize cache")?;

    fs::write(&cache_path, j)
        .with_context(|| format!("Failed to write cache file to {}", cache_path.display()))?;

    Ok(())
}

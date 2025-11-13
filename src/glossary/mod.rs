mod chapter;
mod data;
mod text;
mod util;

pub use chapter::get_last_chapter_number;

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    time::Instant,
};

use aho_corasick::AhoCorasick;
use anyhow::{Context, Result, anyhow};
use clipboard_win::set_clipboard_string;
use itertools::Itertools;
use jieba_rs::Jieba;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use tokio::fs::read_to_string;

use crate::config::get_config;
use chapter::*;
use data::*;
use text::*;
#[allow(unused)]
use util::*;

#[derive(Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Clone)]
pub struct GlossaryEntry {
    pub en: String,
    pub cn: String,
    pub pinyin: String,
    #[serde(rename = "type")]
    pub _type: String,
    pub gender: Option<String>,
    pub file: Option<i32>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Chapter {
    pub expected_number: usize,
    pub expected_title: String,
    pub text: String,
}

pub struct GlossaryProcessor {
    jieba: Jieba,
    glossary_data: Vec<GlossaryEntry>,
    original_to_clean_map: HashMap<String, String>,
    valid_clean_terms: Vec<String>,
    ac: AhoCorasick,
    assets_path: PathBuf,
    translations_path: PathBuf,
}

impl GlossaryProcessor {
    pub async fn new(
        assets_path: impl AsRef<Path>,
        translations_path: impl AsRef<Path>,
    ) -> Result<Self> {
        let assets_path = assets_path.as_ref().to_owned();
        let translations_path = translations_path.as_ref().to_owned();

        let jieba = Jieba::new();

        let glossary_data = read_glossary(assets_path.join(&get_config().await.glossary_file))
            .context("Failed to read glossary file. Check if it exists and you have the permission to READ.")?;

        let (original_to_clean_map, valid_clean_terms) = preprocess_glossary(&glossary_data);

        println!("Loaded {} valid glossary entries.", glossary_data.len());

        let ac =
            AhoCorasick::new(&valid_clean_terms).context("Failed to build Aho-Corasick Engine")?;

        Ok(Self {
            jieba,
            glossary_data,
            original_to_clean_map,
            valid_clean_terms,
            ac,
            assets_path,
            translations_path,
        })
    }

    pub async fn process_new_chapters(&self) -> Result<()> {
        let config = get_config().await;

        let chapter_file_path = self.assets_path.join(&config.chapter_file);
        let chapter_file = read_to_string(chapter_file_path)
            .await
            .context("Failed to read chapter file.")?;

        let last_chapter_number = get_last_chapter_number(&self.translations_path)
            .context("Failed to get last chapter number")?;

        let chapters = process_chapters(&chapter_file, last_chapter_number);

        if chapters.is_empty() {
            return Ok(());
        }

        let combined_chapter_text = chapters.iter().map(|c| &c.text).join("\n\n--\n\n");
        let processed_chapter_text = preprocess_chinese_text(&combined_chapter_text);

        let mut time = Instant::now();

        let exact_matches =
            aho_corasick_find_all(&self.ac, &self.valid_clean_terms, &processed_chapter_text);

        println!("Terms found:");

        println!(
            "\tExact: {} [{}ms]",
            exact_matches.len(),
            time.elapsed().as_millis(),
        );

        // Filter out terms we already found
        let terms_for_fuzzy_match: Vec<_> = self
            .valid_clean_terms
            .iter()
            .filter(|term| !exact_matches.contains(term.as_str()))
            .map(|s| s.as_str())
            .collect();

        time = Instant::now();

        let fuzzy_matches = chinese_fuzzy_search(
            &self.jieba,
            &terms_for_fuzzy_match,
            &processed_chapter_text,
            Some(config.fuzzy_search_threshold),
        );

        println!(
            "\tClose: {} [{}ms]",
            fuzzy_matches.len(),
            time.elapsed().as_millis(),
        );

        let mut all_found_terms = HashSet::new();

        all_found_terms.extend(exact_matches);
        all_found_terms.extend(fuzzy_matches);

        println!(
            "Total unique glossary terms after all phases: {}.",
            all_found_terms.len()
        );

        let found_entries: Vec<&GlossaryEntry> = self
            .glossary_data
            .par_iter()
            .filter_map(|entry| {
                let clean_term = self.original_to_clean_map.get(&entry.cn)?;
                if all_found_terms.contains(clean_term.as_str()) {
                    Some(entry)
                } else {
                    None
                }
            })
            .collect();

        println!("\n\n--- Final Micro-Glossary Terms ---\n");

        for entry in &found_entries {
            println!("* {} - {}", entry.cn, entry.en);
        }

        let micro_glossary_string = create_micro_glossary(&found_entries);

        let prompt_template =
            read_to_string(self.assets_path.join(&config.translation_prompt_file))
                .await
                .context("Failed to read translation prompt file")?;

        let final_prompt_string = format!(
            "{}\n\n**Glossary**\n\n{}\n\n---\n\n**Chinese Chapter(s) to Translate:**\n{}",
            prompt_template, micro_glossary_string, combined_chapter_text
        );

        set_clipboard_string(&final_prompt_string)
            .map_err(|e| anyhow!("Failed to set clipboard: {}", e))?;

        create_and_open_files(&self.translations_path, &chapters).await?;

        Ok(())
    }
}

mod chapter;
mod data;
mod text;
mod types;

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    time::Instant,
};

use aho_corasick::AhoCorasick;
use anyhow::{Context, Result};
use clipboard_win::set_clipboard_string;
use itertools::Itertools;
use jieba_rs::Jieba;
use log::info;
use tokio::fs::read_to_string;

use crate::{config::get_config, util::*};
use chapter::*;
use data::*;
use text::*;
use types::*;

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

        info!("Loading Chinese segmenter dictionary...");
        let jieba = Jieba::new();
        info!("Dictionary loaded.");

        info!("Loading glossary...");
        let glossary_data = read_glossary(assets_path.join(&get_config().await.glossary_file))
            .context("Failed to read glossary file")?;

        info!("Preprocessing glossary...");
        // The clean_to_original_map is temporary here, used only to build valid_clean_terms
        let (original_to_clean_map, clean_to_original_map) = preprocess_glossary(&glossary_data);

        let valid_clean_terms: Vec<String> = clean_to_original_map.keys().cloned().collect();

        info!("Loaded {} glossary entries.", glossary_data.len());
        info!("{} are valid for searching.", valid_clean_terms.len());

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
        let chapter_file_path = self.assets_path.join(&get_config().await.chapter_file);
        let chapter_file = read_to_string(chapter_file_path).await.context(format!(
            "Failed to read chapter file: {}",
            &get_config().await.chapter_file
        ))?;

        info!(
            "Scanning for chapters in {}",
            &get_config().await.chapter_file
        );

        let last_chapter_number = get_last_chapter_number(&self.translations_path)
            .context("Failed to get last chapter number")?;
        let chapters = process_chapters(&chapter_file, last_chapter_number);

        if chapters.is_empty() {
            return Ok(());
        }

        let mut combined_chapter_text = String::new();

        for chapter in &chapters {
            combined_chapter_text.push_str(&chapter.text);
            combined_chapter_text.push_str("\n\n---\n\n");
        }

        let processed_chapter_text = preprocess_chinese_text(&combined_chapter_text);

        info!("Starting glossary search...");

        let time = Instant::now();

        let exact_matches =
            aho_corasick_find_all(&self.ac, &self.valid_clean_terms, &processed_chapter_text);
        info!(
            "Phase 1 (Chinese Exact): Found {} unique terms. [{}ms]",
            exact_matches.len(),
            time.elapsed().as_millis(),
        );

        // Filter out terms we already found
        let terms_for_fuzzy_match: Vec<_> = self
            .valid_clean_terms
            .iter()
            .filter(|term| !exact_matches.contains(term.as_str()))
            .cloned()
            .collect();

        let fuzzy_matches = chinese_fuzzy_search(
            &self.jieba,
            &terms_for_fuzzy_match,
            &processed_chapter_text,
            Some(get_config().await.fuzzy_search_threshold),
        );
        info!(
            "Phase 2 (Chinese Fuzzy): Found {} unique terms. [{}ms] ",
            fuzzy_matches.len(),
            time.elapsed().as_millis(),
        );

        let mut all_found_terms = HashSet::new();

        all_found_terms.extend(exact_matches);
        all_found_terms.extend(fuzzy_matches);

        info!(
            "Total unique glossary terms after all phases: {}.",
            all_found_terms.len()
        );

        let found_entries: Vec<_> = self
            .glossary_data
            .iter()
            .filter_map(|entry| {
                let clean_term = self.original_to_clean_map.get(&entry.cn)?;
                if all_found_terms.contains(clean_term) {
                    Some(entry.clone())
                } else {
                    None
                }
            })
            .collect();

        println!("\n\n--- Final Micro-Glossary Terms ---\n");
        for entry in &found_entries {
            println!("* {} - {}", entry.cn, entry.en);
        }

        let micro_glossary_string = found_entries
            .iter()
            .map(|entry| {
                let cn = &entry.cn;
                let pinyin = &entry.pinyin;
                let en = &entry.en;

                let mut details = vec![&entry._type];
                if let Some(gender) = &entry.gender {
                    details.push(gender);
                }
                let details_string = format!("[{}]", details.iter().join(", "));

                format!("* {} ({}) -> {} {}", cn, pinyin, en, details_string)
            })
            .join("\n");

        let prompt_template = read_to_string(
            self.assets_path
                .join(&get_config().await.translation_prompt_file),
        )
        .await
        .context("Failed to read translation prompt file")?;

        let final_prompt_string = format!(
            "{}\n\n**Glossary**\n\n{}\n\n---\n\n**Chinese Chapter(s) to Translate:**\n{}",
            prompt_template, micro_glossary_string, combined_chapter_text
        );

        set_clipboard_string(&final_prompt_string)
            .map_err(|e| anyhow::anyhow!("Failed to set clipboard: {}", e))?;

        let new_files: Vec<_> = chapters
            .iter()
            .map(|chapter| {
                self.translations_path
                    .join(format!("{}{}", chapter.expected_number, ".md"))
            })
            .collect();

        println!();
        create_and_open_files(&new_files).await;

        let separator = "=".repeat(50);
        println!("\n{}", separator);
        info!(
            "SUCCESS! The prompt for {} chapter(s) has been copied to clipboard.",
            chapters.len()
        );
        info!("Total glossary entries: {}", found_entries.len());
        info!("Created/Verified {} file(s)", new_files.len());
        println!("{}", separator);

        Ok(())
    }
}

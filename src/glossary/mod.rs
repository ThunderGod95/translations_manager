mod chapter;
mod data;
mod text;
mod util;

use dialoguer::{Confirm, theme::ColorfulTheme};

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
    time::Instant,
};

use anyhow::{Context, Result, bail};
use itertools::Itertools;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::fs::read_to_string;

use crate::{config::CONFIG, util::is_file_empty};
use chapter::*;
use data::*;
use text::*;
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

struct GlossaryProcessor {
    glossary_data: Vec<GlossaryEntry>,
    original_to_clean_map: HashMap<String, String>,
    valid_clean_terms: Vec<String>,
    assets_path: PathBuf,
    translations_path: PathBuf,
    last_chapter_num: usize,
}

impl GlossaryProcessor {
    fn new(
        assets_path: impl AsRef<Path>,
        translations_path: impl AsRef<Path>,
        last_chapter_num: usize,
    ) -> Result<Self> {
        let assets_path = assets_path.as_ref().to_owned();
        let translations_path = translations_path.as_ref().to_owned();

        let glossary_path = assets_path.join(&CONFIG.glossary_file);
        let glossary_data = read_glossary(glossary_path).context(
            "Failed to read glossary file. Check if it exists and you have the permission to READ.",
        )?;

        let (original_to_clean_map, valid_clean_terms) = preprocess_glossary(&glossary_data);

        println!("Loaded {} valid glossary entries.\n", glossary_data.len(),);

        Ok(Self {
            glossary_data,
            original_to_clean_map,
            valid_clean_terms,
            assets_path,
            translations_path,
            last_chapter_num,
        })
    }

    fn process_new_chapters(&self) -> Result<()> {
        let config = &CONFIG;

        // 1. Read and process chapter text
        let chapter_file_path = self.assets_path.join(&config.chapter_file);
        let chapter_file =
            read_to_string(chapter_file_path).context("Failed to read chapter file.")?;

        let chapters = process_chapters(&chapter_file, self.last_chapter_num)?;

        if chapters.is_empty() {
            return Ok(());
        }

        let combined_chapter_text = chapters.iter().map(|c| &c.text).join("\n\n--\n\n");
        let processed_chapter_text = preprocess_chinese_text(&combined_chapter_text);

        // 2. Run searches to find all unique terms
        let all_found_terms =
            self.find_all_glossary_terms(&processed_chapter_text, config.fuzzy_search_threshold);

        println!(
            "Total unique glossary terms after all phases: {}.",
            all_found_terms.len()
        );

        // 3. Build the micro-glossary
        let (found_entries, micro_glossary_string) = self.build_micro_glossary(&all_found_terms);

        println!("\n\n--- Final Micro-Glossary Terms ---\n");

        for entry in &found_entries {
            println!("* {} - {}", entry.cn, entry.en);
        }

        // 4. Build and paste the final prompt
        let final_prompt_string =
            self.build_final_prompt(&micro_glossary_string, &combined_chapter_text)?;

        paste_glossary(final_prompt_string)?;

        // 5. Create placeholder files for translation
        create_and_open_files(&self.translations_path, &chapters)?;

        Ok(())
    }

    fn find_all_glossary_terms<'a>(
        &'a self,
        text: &'a str,
        fuzzy_threshold: u32,
    ) -> HashSet<String> {
        let mut time = Instant::now();

        let exact_matches = aho_corasick_find_all(&self.valid_clean_terms, text);

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

        let fuzzy_matches =
            chinese_fuzzy_search(&terms_for_fuzzy_match, text, Some(fuzzy_threshold));

        println!(
            "\tClose: {} [{}ms]",
            fuzzy_matches.len(),
            time.elapsed().as_millis(),
        );

        let mut all_found_terms: HashSet<String> =
            exact_matches.into_iter().map(String::from).collect();
        all_found_terms.extend(fuzzy_matches.into_iter().map(String::from));

        all_found_terms
    }

    fn build_micro_glossary<'a>(
        &'a self,
        all_found_terms: &HashSet<String>,
    ) -> (Vec<&'a GlossaryEntry>, String) {
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

        let micro_glossary_string = create_micro_glossary(&found_entries);

        (found_entries, micro_glossary_string)
    }

    /// Reads the prompt template and combines it with the glossary and chapter text.
    fn build_final_prompt(
        &self,
        micro_glossary_string: &str,
        combined_chapter_text: &str,
    ) -> Result<String> {
        let config = &CONFIG;
        let prompt_template =
            read_to_string(self.assets_path.join(&config.translation_prompt_file))
                .context("Failed to read translation prompt file")?;

        Ok(format!(
            "{}\n\n**Glossary**\n\n{}\n\n---\n\n**Chinese Chapter(s) to Translate:**\n{}",
            prompt_template, micro_glossary_string, combined_chapter_text
        ))
    }
}

pub fn glossary_processor(
    assets_path: impl AsRef<Path>,
    translations_path: impl AsRef<Path>,
) -> Result<()> {
    let last_chapter = find_last_chapter(&translations_path)?;
    let last_chapter_path = translations_path
        .as_ref()
        .join(format!("{}.md", last_chapter));

    if is_file_empty(last_chapter_path) {
        let con = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt("Previous chapter file is empty. Do you still want to proceed?")
            .default(false)
            .show_default(true)
            .interact()?;

        if !con {
            bail!("Chapter no. mismatch found. Exiting...");
        }
    }

    let glossary_processor = GlossaryProcessor::new(assets_path, translations_path, last_chapter)?;

    glossary_processor.process_new_chapters()?;

    Ok(())
}

mod chapter;
mod data;
mod util;

use std::fs::read_to_string;

use anyhow::{Context, Result, bail};
use dialoguer::{Confirm, theme::ColorfulTheme};
use itertools::Itertools;

use crate::{
    config::{CONFIG, ProjectPaths},
    core::glossary::{GlossaryEntry, GlossaryOptions, create_micro_glossary_with_options},
    util::is_file_empty,
};

use chapter::*;
use data::*;
use util::*;

pub use chapter::find_last_chapter;

#[derive(Debug, PartialEq, Eq)]
pub struct Chapter {
    pub expected_number: usize,
    pub expected_title: String,
    pub text: String,
    pub create_file: bool,
}

#[derive(Debug)]
struct GlossaryProcessor {
    glossary_data: Vec<GlossaryEntry>,
    paths: ProjectPaths,
    last_chapter_num: usize,
    write_raw: bool,
}

impl GlossaryProcessor {
    fn new(paths: ProjectPaths, last_chapter_num: usize, write_raw: bool) -> Result<Self> {
        /*
         * Read only the configuration value we need and immediately release
         * the global config lock.
         */
        let glossary_file = {
            let config = CONFIG.read().expect("Config lock poisoned");

            config.glossary_file.clone()
        };

        let glossary_path = paths.assets_folder.join(glossary_file);

        let glossary_data = read_glossary(glossary_path).context(
            "Failed to read glossary file. \
                 Check if it exists and you have permission to read it.",
        )?;

        println!("Loaded {} valid glossary entries.\n", glossary_data.len());

        Ok(Self {
            glossary_data,
            paths,
            last_chapter_num,
            write_raw,
        })
    }

    fn process_new_chapters(&self) -> Result<()> {
        let (chapter_file, fuzzy_threshold) = {
            let config = CONFIG.read().expect("Config lock poisoned");

            (config.chapter_file.clone(), config.fuzzy_search_threshold)
        };

        let chapter_file_path = self.paths.assets_folder.join(chapter_file);

        let chapter_file =
            read_to_string(chapter_file_path).context("Failed to read chapter file.")?;

        let chapters = process_chapters(&chapter_file, self.last_chapter_num)?;

        if chapters.is_empty() {
            return Ok(());
        }

        let combined_chapter_text = chapters
            .iter()
            .map(|chapter| &chapter.text)
            .join("\n\n--\n\n");

        let found_entries = create_micro_glossary_with_options(
            &combined_chapter_text,
            &self.glossary_data,
            GlossaryOptions {
                fuzzy: true,
                fuzzy_threshold,
            },
        )
        .context("Failed to generate micro glossary")?;

        println!(
            "Total unique glossary terms after all phases: {}.",
            found_entries.len()
        );

        println!("\n\n--- Final Micro-Glossary Terms ---\n");

        for entry in &found_entries {
            if let Some(summary) = &entry.summary {
                println!("* {} - {} ({})", entry.cn, entry.en, summary);
            } else {
                println!("* {} - {}", entry.cn, entry.en);
            }
        }

        let micro_glossary_string = format_micro_glossary(&found_entries);

        let final_prompt_string =
            self.build_final_prompt(&micro_glossary_string, &combined_chapter_text)?;

        paste_glossary(&final_prompt_string)?;

        if self.write_raw {
            write_raws(&self.paths.raws_folder, &chapters);
        }

        create_and_open_files(&self.paths.translations_folder, &chapters)?;

        Ok(())
    }

    /// Reads the prompt template and combines it with the glossary and
    /// chapter text.
    fn build_final_prompt(
        &self,
        micro_glossary_string: &str,
        combined_chapter_text: &str,
    ) -> Result<String> {
        let translation_prompt_file = {
            let config = CONFIG.read().expect("Config lock poisoned");

            config.translation_prompt_file.clone()
        };

        let prompt_template =
            read_to_string(self.paths.assets_folder.join(translation_prompt_file))
                .context("Failed to read translation prompt file")?;

        Ok(format!(
            "{}\n\n\
             **Glossary**\n\n\
             {}\n\n\
             ---\n\n\
             **Chinese Chapter(s) to Translate:**\n\
             {}",
            prompt_template, micro_glossary_string, combined_chapter_text
        ))
    }
}

pub fn glossary_processor(project_name: &str, write_raw: bool) -> Result<()> {
    let paths = ProjectPaths::new(project_name);

    let translations_path = &paths.translations_folder;

    let last_chapter = find_last_chapter(translations_path)?;

    let last_chapter_path = translations_path.join(format!("{}.md", last_chapter));

    if is_file_empty(last_chapter_path) {
        let confirm = Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(
                "Previous chapter file is empty. \
                 Do you still want to proceed?",
            )
            .default(false)
            .show_default(true)
            .interact()?;

        if !confirm {
            bail!("Chapter no. mismatch found. Exiting...");
        }
    }

    let glossary_processor = GlossaryProcessor::new(paths, last_chapter, write_raw)?;

    glossary_processor.process_new_chapters()?;

    Ok(())
}

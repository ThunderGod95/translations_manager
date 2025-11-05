use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use strum::{Display, EnumString, VariantArray};

#[derive(Parser)]
#[command(
    version,
    about = "Tool for managing translation projects.",
    long_about = "A tool to assist with translation workflows, \
                  including glossary generation and content searching."
)]
pub struct Cli {
    /// The command to execute (e.g., 'glossary', 'find')
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Name of the translation project to operate on
    #[arg(short, long, value_name = "PROJECT_NAME")]
    pub project: Option<String>,

    /// Sets the base directory where all translation projects are located
    #[arg(long, value_name = "PATH")]
    pub path: Option<PathBuf>,
}

#[derive(Debug, Subcommand, Clone, PartialEq, Eq)]
pub enum Command {
    /// Generate a micro-glossary for an untranslated chapter
    #[command(
        name = "glossary",
        long_about = "Generates a micro-glossary based on words found in an untranslated \
                      chapter. It copies the glossary, translation prompt, and chapter \
                      content to the clipboard."
    )]
    Glossary,

    /// Search translated chapters for a specific text pattern
    #[command(
        name = "find",
        long_about = "Finds the first occurrence and all occurrences of a pattern (text or regex) \
                      within a specified range of files."
    )]
    Find(FindArgs),

    /// Find and replace a pattern in translated chapters
    #[command(
        name = "replace",
        long_about = "Finds and replaces occurrences of a pattern (text or regex) \
                      with new text within a specified range of files."
    )]
    Replace(ReplaceArgs),

    #[command(
        name = "distribute",
        long_about = "Bundles up all translated chapters into EPUBs and PDFs by volumes (as specified in sep.json)."
    )]
    Distribute,

    #[command(name = "open", long_about = "Open specified chapter(s) in VS Code.")]
    Open(OpenArgs),

    /// Edit Config files.
    #[command(
        name = "internal",
        long_about = "Opens the internal config files in VS code for editing. Will fail if VS code is not installed."
    )]
    Internal,
}

impl Command {
    pub fn from_task(task: Task) -> Self {
        match task {
            Task::Glossary => Command::Glossary,
            Task::Find => Command::Find(FindArgs::default()),
            Task::Replace => Command::Replace(ReplaceArgs::default()),
            Task::Distribute => Command::Distribute,
            Task::Open => Command::Open(OpenArgs::default()),
            Task::Internal => Command::Internal,
        }
    }
}

#[derive(Debug, Clone, Args, PartialEq, Eq, Default)]
pub struct FindArgs {
    /// The text or regular expression to search for
    pub pattern: Option<String>,

    /// File to write all matching paragraphs to (optional)
    #[arg(short, long)]
    pub write: Option<PathBuf>,

    /// The chapter number to start the search from (inclusive)
    #[arg(long)]
    pub start: Option<usize>,

    /// The chapter number to end the search at (inclusive)
    #[arg(long)]
    pub end: Option<usize>,

    /// Treat the search pattern as a regular expression
    #[arg(required = false, short, long, action = clap::ArgAction::SetTrue)]
    pub regex: bool,

    /// Suppress the detailed table output, showing only the summary
    #[arg(required = false, short, long, action = clap::ArgAction::SetTrue)]
    pub silent: bool,
}

#[derive(Debug, Clone, Args, PartialEq, Eq, Default)]
pub struct ReplaceArgs {
    /// The text or regular expression to replace
    pub old: Option<String>,
    /// The text to replace with
    pub new: Option<String>,
    /// Treat the search pattern as a regular expression
    #[arg(required = false, short, long, action = clap::ArgAction::SetTrue)]
    pub regex: bool,
}

#[derive(Debug, Clone, Args, PartialEq, Eq, Default)]
pub struct OpenArgs {
    /// Chapters to open in VS Code.
    pub files: Option<Vec<usize>>,
}

#[derive(Debug, Clone, Copy, VariantArray, EnumString, Display)]
#[strum(serialize_all = "lowercase")]
pub enum Task {
    /// The glossary generation task
    Glossary,
    /// The pattern finding task
    Find,
    /// The pattern replacing task
    Replace,
    /// The distribution task
    Distribute,
    /// The chapter opening task
    Open,
    /// Editing the config task
    Internal,
}

pub mod args {
    use std::path::PathBuf;

    use anyhow::Result;
    use dialoguer::{Confirm, History, Input, theme::ColorfulTheme};

    use crate::{
        commands::{FindArgs, ReplaceArgs, args::histories::FindHistory},
        util::get_find_history_config_path,
    };

    use super::Command;

    /// Currently just a placeholder for future commands.
    pub fn populate_arguments(command: &mut Command) -> Result<()> {
        match command {
            Command::Glossary => Ok(()),
            Command::Internal => Ok(()),
            Command::Distribute => Ok(()),
            Command::Open(_) => Ok(()),
            Command::Find(find_args) => populate_find_arguments(find_args),
            Command::Replace(replace_args) => populate_replace_arguments(replace_args),
        }
    }

    pub fn populate_find_arguments(find_args: &mut FindArgs) -> Result<()> {
        let mut history = FindHistory::load(get_find_history_config_path()?, 20);

        if let Some(pattern) = &find_args.pattern {
            history.write(pattern);
            return Ok(());
        }

        let theme = ColorfulTheme::default();

        let pattern: String = Input::with_theme(&theme)
            .with_prompt("Enter search pattern (required)")
            .history_with(&mut history)
            .interact_text()?;

        let start_num: String = Input::with_theme(&theme)
            .with_prompt("Enter start file number (optional)")
            .allow_empty(true)
            .interact_text()?;

        let end_num: String = Input::with_theme(&theme)
            .with_prompt("Enter end file number (optional)")
            .allow_empty(true)
            .interact_text()?;

        let write_to: String = Input::with_theme(&theme)
            .with_prompt("Output file to write paragraphs to (optional)")
            .allow_empty(true)
            .interact_text()?;

        let regex = Confirm::with_theme(&theme)
            .with_prompt("Treat the search pattern as a regular expression?")
            .default(false)
            .show_default(true)
            .interact()?;

        let silent = Confirm::with_theme(&theme)
            .with_prompt("Suppress the detailed table output, showing only the summary?")
            .default(false)
            .show_default(true)
            .interact()?;

        find_args.pattern = Some(pattern);
        find_args.start = if !start_num.is_empty() {
            match start_num.parse::<usize>() {
                Ok(num) => Some(num),
                Err(_) => None,
            }
        } else {
            None
        };
        find_args.end = if !end_num.is_empty() {
            match end_num.parse::<usize>() {
                Ok(num) => Some(num),
                Err(_) => None,
            }
        } else {
            None
        };
        find_args.write = if !write_to.is_empty() {
            Some(PathBuf::from(write_to))
        } else {
            None
        };
        find_args.regex = regex;
        find_args.silent = silent;

        Ok(())
    }

    pub fn populate_replace_arguments(replace_args: &mut ReplaceArgs) -> Result<()> {
        let theme = ColorfulTheme::default();

        if replace_args.old.is_none() {
            let old: String = Input::with_theme(&theme)
                .with_prompt("Enter the pattern to replace (required)")
                .allow_empty(false)
                .interact_text()?;

            replace_args.old = Some(old);
        }

        if replace_args.new.is_none() {
            let new: String = Input::with_theme(&theme)
                .with_prompt("Enter the replacement (required)")
                .allow_empty(false)
                .interact_text()?;

            replace_args.new = Some(new);
        }

        let regex = Confirm::with_theme(&theme)
            .with_prompt("Treat the search pattern as a regular expression?")
            .default(false)
            .show_default(true)
            .interact()?;

        replace_args.regex = regex;

        Ok(())
    }

    mod histories {
        use std::{
            collections::VecDeque,
            fs::{File, OpenOptions},
            io::{self, BufRead, BufReader, Write},
            path::PathBuf,
        };

        use dialoguer::History;

        pub struct FindHistory {
            max: usize,
            path: PathBuf,
            history: VecDeque<String>,
        }

        impl FindHistory {
            pub fn load(path: PathBuf, max: usize) -> Self {
                let history = if let Ok(file) = File::open(&path) {
                    let reader = BufReader::new(file);
                    reader
                        .lines()
                        .filter_map(Result::ok)
                        .take(max)
                        .collect::<VecDeque<String>>()
                } else {
                    VecDeque::new()
                };

                Self { max, path, history }
            }

            fn save(&self) -> io::Result<()> {
                let mut file = OpenOptions::new()
                    .write(true)
                    .create(true)
                    .truncate(true)
                    .open(&self.path)?;

                for item in &self.history {
                    writeln!(file, "{}", item)?;
                }
                Ok(())
            }
        }

        impl<T: ToString> History<T> for FindHistory {
            fn read(&self, pos: usize) -> Option<String> {
                self.history.get(pos).cloned()
            }

            fn write(&mut self, val: &T) {
                let s = val.to_string();

                if let Some(pos) = self.history.iter().position(|x| x == &s) {
                    self.history.remove(pos);
                }

                self.history.push_front(s);

                if self.history.len() > self.max {
                    self.history.pop_back();
                }

                self.save().ok();
            }
        }
    }
}

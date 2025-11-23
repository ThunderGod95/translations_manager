use std::path::PathBuf;

use anyhow::Result;
use dialoguer::{Confirm, History, Input, theme::ColorfulTheme};

use super::cli::{Command, FindArgs, InitArgs, ReplaceArgs};
use crate::{runner::interactive::histories::FindHistory, util::get_find_history_config_path};

pub fn populate_arguments(command: &mut Command) -> Result<()> {
    match command {
        Command::Glossary => Ok(()),
        Command::Internal => Ok(()),
        Command::Distribute(_) => Ok(()),
        Command::Open(_) => Ok(()),
        Command::Find(find_args) => populate_find_arguments(find_args),
        Command::Replace(replace_args) => populate_replace_arguments(replace_args),
        Command::Init(init_args) => populate_init_arguments(init_args),
        Command::Next => Ok(()),
    }
}

pub fn populate_find_arguments(find_args: &mut FindArgs) -> Result<()> {
    let history_path = get_find_history_config_path()?;
    let mut history = FindHistory::load(history_path, 20);

    if let Some(pattern) = &find_args.pattern {
        history.write(pattern);
        return Ok(());
    }

    let theme = ColorfulTheme::default();

    let pattern: String = Input::with_theme(&theme)
        .with_prompt("Enter search pattern (required)")
        .history_with(&mut history)
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

    find_args.pattern = Some(pattern);
    find_args.write = if !write_to.is_empty() {
        let silent = Confirm::with_theme(&theme)
            .with_prompt("Suppress the detailed table output, showing only the summary?")
            .default(false)
            .show_default(true)
            .interact()?;

        find_args.silent = silent;
        Some(PathBuf::from(write_to))
    } else {
        None
    };
    find_args.regex = regex;

    Ok(())
}

pub fn populate_replace_arguments(replace_args: &mut ReplaceArgs) -> Result<()> {
    let theme = ColorfulTheme::default();

    if replace_args.old.is_some() && replace_args.new.is_some() {
        return Ok(());
    }

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

pub fn populate_init_arguments(init_args: &mut InitArgs) -> Result<()> {
    if init_args.project_name.is_none() {
        let project_name: String = Input::with_theme(&ColorfulTheme::default())
            .with_prompt("Please enter the name:")
            .allow_empty(false)
            .interact_text()?;

        init_args.project_name = Some(project_name);
    }

    Ok(())
}

mod histories {
    use std::{
        collections::VecDeque,
        fs::{File, OpenOptions},
        io::{self, BufRead, BufReader, Write},
        path::{Path, PathBuf},
    };

    use dialoguer::History;

    pub struct FindHistory {
        max: usize,
        path: PathBuf,
        history: VecDeque<String>,
    }

    impl FindHistory {
        pub fn load(path: impl AsRef<Path>, max: usize) -> Self {
            let path = path.as_ref().to_path_buf();

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

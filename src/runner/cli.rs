use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use clap_verbosity_flag::{InfoLevel, Verbosity};
use strum::{Display, EnumString, VariantArray};

#[derive(Parser, Debug, Clone)]
#[command(
    version,
    about = "Tool for managing translation projects.",
    long_about = "A tool to assist with translation workflows, \
                  including glossary generation and content searching."
)]
pub struct Cli {
    #[clap(flatten)]
    pub verbose: Verbosity<InfoLevel>,

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
        long_about = "Bundles up all translated chapters into EPUBs and PDFs by volumes (as specified in sep.json) using pandoc."
    )]
    Distribute(DistArgs),

    #[command(
        name = "open",
        long_about = "Open specified chapter(s) or project in VS Code."
    )]
    Open(OpenArgs),

    #[command(name = "init", long_about = "Initialize a new translation project.")]
    Init(InitArgs),

    /// Edit Config files.
    #[command(
        name = "internal",
        long_about = "Opens the internal application config files in VS code for editing. Will fail if VS code is not installed."
    )]
    Internal,
}

impl Command {
    pub fn from_task(task: Task) -> Self {
        match task {
            Task::Glossary => Command::Glossary,
            Task::Find => Command::Find(FindArgs::default()),
            Task::Replace => Command::Replace(ReplaceArgs::default()),
            Task::Distribute => Command::Distribute(DistArgs::default()),
            Task::Open => Command::Open(OpenArgs::default()),
            Task::Init => Command::Init(InitArgs::default()),
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
pub struct DistArgs {
    /// Distribute as TXT. Specifying this option will disable generation of other formats.
    #[arg(short, long, required = false)]
    pub txt: bool,
}

#[derive(Debug, Clone, Args, PartialEq, Eq, Default)]
pub struct OpenArgs {
    pub files: Option<Vec<usize>>,
}

#[derive(Debug, Clone, Args, PartialEq, Eq, Default)]
pub struct InitArgs {
    pub project_name: Option<String>,
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
    Init,
    /// Editing the config task
    Internal,
}

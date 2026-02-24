use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use strum::{Display, EnumString, VariantArray};

use crate::distribute::DistributionFormat;

#[derive(Parser, Debug, Clone)]
#[command(
    version,
    about = "Tool for managing translation projects.",
    long_about = "A tool to assist with translation workflows, \
                  including glossary generation and content searching."
)]
pub struct Cli {
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
    /// Clean all chapters for known errors.
    #[command(
        name = "clean",
        long_about = "Scans all translated chapters to detect and correct common formatting errors.
    This process standardizes punctuation, fixes whitespace inconsistencies,
    and removes known artifacts to ensure a clean text."
    )]
    Clean(CleanArgs),

    /// Compile translated chapters into distributable formats (EPUB, PDF)
    #[command(
        name = "distribute",
        long_about = "Compiles all translated chapters into final EPUB and PDF files.
This command uses Pandoc to create book-like volumes, with chapter
separation defined in the 'sep.json' configuration file."
    )]
    Distribute(DistArgs),

    /// Search translated chapters for text or regex patterns
    #[command(
        name = "find",
        long_about = "Searches for a text or regex pattern within a specified range of
translated chapter files. Reports the first occurrence and a total
count of all matches."
    )]
    Find(FindArgs),

    /// Prepare an untranslated chapter for translation
    #[command(
        name = "glossary",
        long_about = "Prepares the next untranslated chapter for translation by:
1. Generating a micro-glossary from its content.
2. Copying the glossary, translation prompt, and chapter text to the clipboard.
3. Creating empty placeholder files for all other untranslated chapters."
    )]
    Glossary,

    /// Initialize a new translation project directory
    #[command(
        name = "init",
        long_about = "Sets up a new translation project in the current directory.
This creates the necessary directory structure (e.g., 'source',
'translated') and default configuration files to get started."
    )]
    Init(InitArgs),

    /// Edit internal application configuration files
    #[command(
        name = "internal",
        long_about = "Opens the internal application configuration files in the preferred editor for
manual editing."
    )]
    Internal,

    #[command(
        name = "nav",
        long_about = "Add navigation links to the specificed chapter."
    )]
    Nav(NavArgs),

    /// Runs the glossary command for the next untranslated chapter
    #[command(
        name = "next",
        long_about = "Automatically identifies the next missing chapter in the translation sequence.
        If the corresponding raw source file is found, it immediately triggers the
        preparation workflow (glossary generation and context setup) for that chapter."
    )]
    Next,

    /// Open project files or chapters in the preferred editor.
    #[command(
        name = "open",
        long_about = "Quickly opens specified chapter files (translated) or
the entire project root directory in the preferred editor."
    )]
    Open(OpenArgs),

    #[command(name = "print", long_about = "")]
    Print,

    /// Find and replace text or regex patterns in translated chapters
    #[command(
        name = "replace",
        long_about = "Finds and replaces all occurrences of a pattern (text or regex)
with new text within a specified range of translated chapter files."
    )]
    Replace(ReplaceArgs),
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
            Task::Next => Command::Next,
            Task::Clean => Command::Clean(CleanArgs::default()),
            Task::Nav => Command::Nav(NavArgs::default()),
            Task::Print => Command::Print,
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
    /// Specify output formats. If left empty, all formats will be generated.
    /// Example: --formats epub pdf
    #[arg(short, long, value_enum)]
    pub formats: Vec<DistributionFormat>,
    /// Specify the volumes to process. If left empty, all volumes will be processed.
    /// Example: --volumes 1 2
    #[arg(short, long)]
    pub volumes: Vec<usize>,
}

#[derive(Debug, Clone, Args, PartialEq, Eq, Default)]
pub struct OpenArgs {
    pub files: Option<Vec<usize>>,
}

#[derive(Debug, Clone, Args, PartialEq, Eq, Default)]
pub struct InitArgs {
    pub project_name: Option<String>,
}

#[derive(Debug, Clone, Args, PartialEq, Eq, Default)]
pub struct CleanArgs {
    pub file: Option<usize>,
    /// Add YAML Front Matter instead of navigation links.
    #[arg(required = false, short, long, action = clap::ArgAction::SetTrue)]
    pub yaml: bool,
    /// If specified, filenames will not be padded with 0s
    #[arg(required = false, short, long, action = clap::ArgAction::SetFalse)]
    pub pad: bool,
}

#[derive(Debug, Clone, Args, PartialEq, Eq, Default)]
pub struct NavArgs {
    pub chapter: u32,
}

#[derive(Debug, Clone, Copy, VariantArray, EnumString, Display)]
#[strum(serialize_all = "lowercase")]
pub enum Task {
    /// The clean task
    Clean,
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
    /// The navigation links task
    Nav,
    /// The next chapter glossary task
    Next,
    /// Initializing new project task
    Init,
    /// Editing the config task
    Internal,
    /// Print base projects path and all known projects to console.
    Print,
}

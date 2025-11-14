use std::path::PathBuf;

use clap::{Args, Parser, Subcommand};
use strum::{Display, EnumString, VariantArray};

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
    /// 📚 Compile translated chapters into distributable formats (EPUB, PDF)
    #[command(
        name = "distribute",
        long_about = "Compiles all translated chapters into final EPUB and PDF files.
This command uses Pandoc to create book-like volumes, with chapter 
separation defined in the 'sep.json' configuration file."
    )]
    Distribute(DistArgs),

    /// 🔎 Search translated chapters for text or regex patterns
    #[command(
        name = "find",
        long_about = "Searches for a text or regex pattern within a specified range of 
translated chapter files. Reports the first occurrence and a total 
count of all matches."
    )]
    Find(FindArgs),

    /// 🖋️ Prepare an untranslated chapter for translation
    #[command(
        name = "glossary",
        long_about = "Prepares the next untranslated chapter for translation by:
1. Generating a micro-glossary from its content.
2. Copying the glossary, translation prompt, and chapter text to the clipboard.
3. Creating empty placeholder files for all other untranslated chapters."
    )]
    Glossary,

    /// ✨ Initialize a new translation project directory
    #[command(
        name = "init",
        long_about = "Sets up a new translation project in the current directory.
This creates the necessary directory structure (e.g., 'source', 
'translated') and default configuration files to get started."
    )]
    Init(InitArgs),

    /// ⚙️ Edit internal application configuration files
    #[command(
        name = "internal",
        long_about = "Opens the internal application configuration files in VS Code for 
manual editing. This command requires VS Code to be installed 
and available in the system's PATH."
    )]
    Internal,

    /// 📖 Open project files or chapters in VS Code
    #[command(
        name = "open",
        long_about = "Quickly opens specified chapter files (source or translated) or 
the entire project root directory in Visual Studio Code."
    )]
    Open(OpenArgs),

    /// 🔁 Find and replace text or regex patterns in translated chapters
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

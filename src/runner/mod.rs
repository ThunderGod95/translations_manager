//! This module handles command-line parsing, argument population,
//! and task execution.

pub mod cli;
pub mod interactive;
pub mod tasks;

pub use cli::{Cli, Command};
pub use interactive::populate_arguments;
pub use tasks::{
    run_dist_task, run_find_task, run_glossary_task, run_init_task, run_internal_task,
    run_open_task, run_replace_task,
};

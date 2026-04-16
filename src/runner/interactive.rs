use anyhow::Result;
use dialoguer::{Input, theme::ColorfulTheme};

use super::cli::{Command, InitArgs};

pub fn populate_arguments(command: &mut Command) -> Result<()> {
    match command {
        Command::Glossary => Ok(()),
        Command::Internal => Ok(()),
        Command::Distribute(_) => Ok(()),
        Command::Open(_) => Ok(()),
        Command::Init(init_args) => populate_init_arguments(init_args),
        Command::Next => Ok(()),
        Command::Clean(_) => Ok(()),
    }
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

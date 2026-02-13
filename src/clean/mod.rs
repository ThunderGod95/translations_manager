mod chapter;
mod formatter;
mod pipeline;

use anyhow::Result;

use crate::{
    clean::{
        formatter::*,
        pipeline::{CleanOptions, Pipeline},
    },
    config::CONFIG,
};

pub fn clean_project(project: &str, use_yaml: bool, pad: bool) -> Result<()> {
    let translations_path = CONFIG.read().unwrap().get_translations_dir(project);

    let mode_name = if use_yaml {
        "YAML Front Matter"
    } else {
        "Navigation Links"
    };
    println!("Selected Mode: {}", mode_name);

    let formatter: Box<dyn formatter::ChapterFormatter> = if use_yaml {
        Box::new(YamlFormatter)
    } else {
        Box::new(NavLinkFormatter)
    };

    let mut options = CleanOptions::new(translations_path, formatter);

    if !pad {
        options.pad = pad;
        println!("Are filenames padded? {pad}");
    }

    Pipeline::execute(options)
}

pub fn sanitize_chapter(content: &str) -> String {
    NavLinkFormatter.clean_existing(&YamlFormatter.clean_existing(content))
}

use super::chapter::Chapter;
use fancy_regex::Regex;
use std::{fmt::Debug, sync::LazyLock};

static RE_NAV_LINKS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)\n*<!--\s*NAV START\s*-->.*?<!--\s*NAV END\s*-->\n*")
        .expect("Invalid regex pattern for chapter parsing")
});
static NAV_START: &'static str = "<!-- NAV START -->";
static NAV_END: &'static str = "<!-- NAV END -->";
static TOC_LINK: &'static str = "[TOC](./)";
static RE_YAML_FRONT_MATTER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)^---\s*\n.*?\n---\s*(\n|$)").expect("Invalid regex pattern for front matter.")
});

/// Context passed to the formatter to help it decide links/padding.
pub struct RenderContext {
    pub prev_id: Option<u32>,
    pub next_id: Option<u32>,
    pub width: usize,
}

pub trait ChapterFormatter: Send + Sync + Debug {
    fn format(&self, chapter: &Chapter, ctx: RenderContext) -> String;
    fn clean_existing(&self, content: &str) -> String;
}

#[derive(Debug)]
pub struct NavLinkFormatter;

impl ChapterFormatter for NavLinkFormatter {
    fn clean_existing(&self, content: &str) -> String {
        let no_bom = content.trim_start_matches('\u{FEFF}');
        let no_yaml = RE_YAML_FRONT_MATTER.replace(no_bom, "");
        RE_NAV_LINKS.replace_all(&no_yaml, "").trim().to_string()
    }

    fn format(&self, chapter: &Chapter, ctx: RenderContext) -> String {
        let clean_body = self.clean_existing(&chapter.content);

        let raw_content = format!(
            "# Chapter {}: {}\n\n{}",
            chapter.number, chapter.title, clean_body
        );

        let make_link = |id: Option<u32>, text: &str| -> String {
            match id {
                Some(n) => format!("[{}](./{:0>width$}.md)", text, n, width = ctx.width),
                None => text.to_string(),
            }
        };

        let prev = make_link(ctx.prev_id, "Previous Chapter");
        let next = make_link(ctx.next_id, "Next Chapter");

        let nav_bar = format!("{NAV_START}\n{prev} | {TOC_LINK} | {next}\n{NAV_END}");

        format!("{nav_bar}\n\n{raw_content}\n\n{nav_bar}")
    }
}

#[derive(Debug)]
pub struct YamlFormatter;

impl ChapterFormatter for YamlFormatter {
    fn clean_existing(&self, content: &str) -> String {
        let no_bom = content.trim_start_matches('\u{FEFF}');
        let no_yaml = RE_YAML_FRONT_MATTER.replace(no_bom, "");
        RE_NAV_LINKS.replace_all(&no_yaml, "").trim().to_string()
    }

    fn format(&self, chapter: &Chapter, ctx: RenderContext) -> String {
        let clean_body = self.clean_existing(&chapter.content);

        let make_val = |id: Option<u32>| match id {
            Some(n) => format!("{:0>width$}", n, width = ctx.width),
            None => "".to_string(),
        };

        let escaped_title = chapter.title.replace("\"", "\\\"");
        let front_matter = format!(
            "---\ntitle: \"{}\"\nid: \"{}\"\nprev: \"{}\"\nnext: \"{}\"\n---\n",
            escaped_title,
            chapter.number,
            make_val(ctx.prev_id),
            make_val(ctx.next_id)
        );

        format!(
            "{}\n{}",
            front_matter,
            format!(
                "# Chapter {}: {}\n\n{}",
                chapter.number, chapter.title, clean_body
            )
        )
    }
}

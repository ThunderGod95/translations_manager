use fancy_regex::Regex;
use std::borrow::Cow;
use std::path::Path;
use std::sync::LazyLock;

use crate::clean::write;

static RE_NAV_LINKS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)\n*<!--\s*NAV START\s*-->.*?<!--\s*NAV END\s*-->\n*")
        .expect("Invalid regex pattern for chapter parsing")
});
static NAV_START: &'static str = "<!-- NAV START -->";
static NAV_END: &'static str = "<!-- NAV END -->";
static TOC_LINK: &'static str = "[TOC](./)";

/// Removes existing navigation blocks from the content.
pub fn remove_nav_links(content: &str) -> String {
    RE_NAV_LINKS.replace_all(content, "").trim().to_string()
}

/// Wraps content with new navigation links.
pub fn wrap_with_nav_links(
    content: &str,
    prev_chap: Option<u32>,
    next_chap: Option<u32>,
    padding: usize,
) -> String {
    let clean_content = write::sanitize_chapter(content);

    let make_link = |chap: Option<u32>, text: &'static str| -> Cow<'static, str> {
        match chap {
            Some(c) => Cow::Owned(format!("[{text}](./{:0>width$}.md)", c, width = padding)),
            None => Cow::Borrowed(text),
        }
    };

    let prev = make_link(prev_chap, "Previous Chapter");
    let next = make_link(next_chap, "Next Chapter");

    let nav_bar = format!("{NAV_START}\n{prev} | {TOC_LINK} | {next}\n{NAV_END}");

    format!("{nav_bar}\n\n{clean_content}\n\n{nav_bar}")
}

/// Calculates the numbers of adjacent chapters based on the list or file system.
pub fn get_adjacent_chapter_numbers(
    base_dir: &Path,
    chapters: &[u32],
    current_idx: usize,
) -> (Option<u32>, Option<u32>) {
    let current_val = chapters.get(current_idx).map(|n| *n);

    let Some(current_num) = current_val else {
        return (None, None);
    };

    let disk_exists = |num: u32| -> bool {
        let path = base_dir.join(format!("{}.md", num));
        let exists = path.exists();
        exists
    };

    let prev = if current_idx > 0 {
        chapters.get(current_idx - 1).map(|n| *n)
    } else {
        None
    };

    let prev = prev.or_else(|| {
        let candidate = current_num.checked_sub(1)?;
        if disk_exists(candidate) {
            Some(candidate)
        } else {
            None
        }
    });

    let next = chapters.get(current_idx + 1).map(|n| *n);

    let next = next.or_else(|| {
        let candidate = current_num + 1;
        if disk_exists(candidate) {
            Some(candidate)
        } else {
            None
        }
    });

    (prev, next)
}

pub mod yaml {
    use crate::clean::write;

    use super::{LazyLock, Regex};

    static RE_FRONT_MATTER: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"(?s)^---\s*\n.*?\n---\s*(\n|$)")
            .expect("Invalid regex pattern for front matter.")
    });

    pub fn remove_front_matter(content: &str) -> String {
        RE_FRONT_MATTER.replace(content, "").to_string()
    }

    pub fn add_front_matter(
        content: &str,
        title: &str,
        num: &str,
        prev_chap: Option<u32>,
        next_chap: Option<u32>,
        padding: usize,
    ) -> String {
        let clean_content = write::sanitize_chapter(content);

        let make_path = |chap: u32| format!("{:0>width$}", chap, width = padding);

        let prev_entry = prev_chap
            .map(|c| format!("prev: \"{}\"\n", make_path(c)))
            .unwrap_or_else(|| "prev: \"\"\n".to_string());

        let next_entry = next_chap
            .map(|c| format!("next: \"{}\"\n", make_path(c)))
            .unwrap_or_else(|| "next: \"\"\n".to_string());

        let escaped_title = title.replace("\"", "\\\"");

        let front_matter = format!(
            "---\ntitle: \"{}\"\nid: \"{}\"\n{}{}---\n",
            escaped_title, num, prev_entry, next_entry
        );

        format!("{}{}", front_matter, clean_content)
    }
}

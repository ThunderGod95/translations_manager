use fancy_regex::Regex;
use std::sync::LazyLock;

/// Represents a chapter parsed from markdown text.
///
/// # Expected Format
/// Chapters should be formatted as:
/// ```markdown
/// # Chapter 1: Introduction
/// Content goes here...
///
/// # Chapter 2: Next Section
/// More content...
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Chapter {
    /// The chapter number as a string slice
    pub number: String,
    /// The chapter title
    pub title: String,
    /// The chapter content (everything after the heading until the next chapter)
    pub content: String,
}

impl Chapter {
    /// Returns the chapter number as a parsed integer, if valid.
    pub fn number_as_usize(&self) -> Option<usize> {
        self.number.parse().ok()
    }
}

static RE_HEADER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^#+\s*Chapter\s*(\d+)\s*:?\s*(.+?)\s*$")
        .expect("Invalid regex pattern for chapter parsing")
});

/// Parses chapters from markdown text.
///
/// Looks for headings in the format: `# Chapter N: Title` or `## Chapter N Title`
/// where N is a number. Captures all content until the next chapter heading.
pub fn parse_chapters(txt: &str) -> Vec<Chapter> {
    let mut matches = RE_HEADER.captures_iter(txt).peekable();
    let mut chapters = Vec::new();

    while let Some(current_cap_result) = matches.next() {
        if let Ok(current_cap) = current_cap_result {
            let number = current_cap.get(1).map_or("", |m| m.as_str()).to_string();
            let title = current_cap.get(2).map_or("", |m| m.as_str()).to_string();

            let start_of_content = current_cap.get(0).unwrap().end();

            let end_of_content = if let Some(Ok(next_cap)) = matches.peek() {
                next_cap.get(0).unwrap().start()
            } else {
                txt.len()
            };

            let content_slice = &txt[start_of_content..end_of_content];

            chapters.push(Chapter {
                number,
                title,
                content: content_slice.trim().to_string(),
            });
        }
    }

    chapters
}

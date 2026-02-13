use fancy_regex::Regex;
use std::sync::LazyLock;

static RE_HEADER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?m)^#+\s*Chapter\s*(\d+)\s*:?\s*(.+?)\s*$")
        .expect("Invalid regex pattern for chapter parsing")
});

#[derive(Debug, Clone, PartialEq)]
pub struct Chapter {
    pub number: String,
    pub title: String,
    pub content: String,
}

impl Chapter {
    pub fn number_as_u32(&self) -> Option<u32> {
        self.number.parse().ok()
    }
}

pub fn parse_chapters(txt: &str) -> Vec<Chapter> {
    let mut matches = RE_HEADER.captures_iter(txt).peekable();
    let mut chapters = Vec::new();

    while let Some(current_cap_result) = matches.next() {
        if let Ok(current_cap) = current_cap_result {
            let number = current_cap.get(1).map_or("", |m| m.as_str()).to_string();
            let title = current_cap.get(2).map_or("", |m| m.as_str()).to_string();
            let start = current_cap.get(0).unwrap().end();
            let end = matches
                .peek()
                .and_then(|r| r.as_ref().ok())
                .map(|m| m.get(0).unwrap().start())
                .unwrap_or(txt.len());

            chapters.push(Chapter {
                number,
                title,
                content: txt[start..end].trim().to_string(),
            });
        }
    }
    chapters
}

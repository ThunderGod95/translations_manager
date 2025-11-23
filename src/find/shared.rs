/// Normalizes a plain string into a case-insensitive regex
/// that respects word boundaries.
pub(super) fn normalize_needle(needle: &str) -> String {
    let starts_with_word = needle
        .chars()
        .next()
        .map_or(false, |c| c.is_alphanumeric() || c == '_');

    let ends_with_word = needle
        .chars()
        .last()
        .map_or(false, |c| c.is_alphanumeric() || c == '_');

    let normalized_search_pattern = fancy_regex::escape(needle);

    let prefix = if starts_with_word { "\\b" } else { "" };
    let suffix = if ends_with_word { "\\b" } else { "" };
    format!("(?i){}{}{}", prefix, normalized_search_pattern, suffix)
}

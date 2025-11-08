use std::borrow::Cow;

use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Eq, Hash, Serialize, Deserialize, Clone)]
pub struct GlossaryEntry {
    pub en: String,
    pub cn: String,
    pub pinyin: String,
    #[serde(rename = "type")]
    pub _type: String,
    pub gender: Option<String>,
    pub file: Option<i32>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct Chapter<'a> {
    pub expected_number: usize,
    pub original_number: usize,
    pub expected_title: String,
    pub original_title: String,
    pub text: Cow<'a, str>,
}


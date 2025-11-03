use std::fmt::Display;
use std::path::Path;

#[derive(Debug, Clone, Default)]
pub struct PandocArgs(Vec<String>);

impl PandocArgs {
    pub fn push_arg(&mut self, arg: impl Display) -> &mut Self {
        self.0.push(format!("{}", arg));
        self
    }

    pub fn set_pdf_engine(&mut self, engine: impl Display) -> &mut Self {
        self.0.push(format!("--pdf-engine={}", engine));
        self
    }

    pub fn set_variable<K: Display, V: Display>(&mut self, key: K, value: V) -> &mut Self {
        self.0.push(format!("--variable={}:{}", key, value));
        self
    }

    pub fn get(&self) -> &Vec<String> {
        &self.0
    }
}

impl Display for PandocArgs {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0.join(" "))
    }
}

#[derive(Debug, Default)]
pub struct PandocMetadata {
    metadata_args: Vec<String>,
    normalized_cover_path: Option<String>,
}

impl PandocMetadata {
    pub fn add<K: Display, V: Display>(&mut self, key: K, value: V) -> &mut Self {
        self.metadata_args
            .push(format!("--metadata={}:{}", key, value));
        self
    }

    pub fn set_cover_image(&mut self, path: impl AsRef<Path>) {
        let clean_path_string = path.as_ref().display().to_string().replace("\\", "/");
        self.normalized_cover_path = Some(clean_path_string);
    }

    pub fn get_cover_image(&self) -> Option<String> {
        self.normalized_cover_path.clone()
    }

    pub fn get_metadata_args(&self) -> &Vec<String> {
        &self.metadata_args
    }
}

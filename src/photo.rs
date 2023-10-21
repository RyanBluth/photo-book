use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct Photo {
    pub path: PathBuf,
}

impl Photo {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub fn file_name(&self) -> &str {
        match self.path.file_name() {
            Some(file_name) => file_name.to_str().unwrap_or("Unknown"),
            None => "Unknown",
        }
    }

    pub fn string_path(&self) -> String {
        self.path.display().to_string()
    }
}

use std::path::PathBuf;

pub struct Session {
    pub active_project: Option<PathBuf>,
}

impl Session {
    pub fn new() -> Self {
        Self {
            active_project: None,
        }
    }
}

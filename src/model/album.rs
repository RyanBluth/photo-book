
use std::path::PathBuf;
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct Album {
    pub name: String,
    pub photos: HashSet<PathBuf>,
}

impl Album {
    pub fn new(name: String, photos: HashSet<PathBuf>) -> Self {
        Self { name, photos }
    }
}
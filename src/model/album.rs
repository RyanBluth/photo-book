use std::collections::HashSet;
use std::path::PathBuf;

pub type AlbumId = String;

#[derive(Debug, Clone)]
pub struct Album {
    pub id: AlbumId,
    pub name: String,
    pub photos: HashSet<PathBuf>,
}

impl Album {
    pub fn new(name: String, photos: HashSet<PathBuf>) -> Self {
        let id = uuid::Uuid::new_v4().to_string();
        Self { id, name, photos }
    }
}

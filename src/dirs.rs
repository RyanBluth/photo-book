use std::path::PathBuf;

use strum::IntoEnumIterator;
use strum_macros::EnumIter;

#[derive(Debug, EnumIter)]
pub enum Dirs {
    Thumbnails,
}

impl Dirs {
    pub fn path(&self) -> PathBuf {
        match *self {
            Dirs::Thumbnails => dirs::cache_dir()
                .unwrap()
                .join("photo_album")
                .join("thumbnails"),
        }
    }
}

pub fn initialize_dirs() {
    for dir in Dirs::iter() {
        let path = dir.path();
        if !path.exists() {
            std::fs::create_dir_all(path).unwrap();
        }
    }
}

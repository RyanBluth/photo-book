use std::path::PathBuf;

use strum::IntoEnumIterator;
use strum_macros::EnumIter;

const SUBDIR: &str = "photo_album";

#[derive(Debug, EnumIter)]
pub enum Dirs {
    Thumbnails,
    Config,
}

impl Dirs {
    pub fn path(&self) -> PathBuf {
        match *self {
            Dirs::Thumbnails => dirs::cache_dir().unwrap().join(SUBDIR),
            Dirs::Config => dirs::config_dir().unwrap().join(SUBDIR),
        }
    }
}

impl Dirs {
    pub fn initialize_dirs() {
        for dir in Dirs::iter() {
            let path = dir.path();
            if !path.exists() {
                std::fs::create_dir_all(path).unwrap();
            }
        }
    }
}

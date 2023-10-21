use std::{
    path::PathBuf,
    sync::{Arc, Mutex, MutexGuard},
};

use crate::{
    dependencies::{Dependency, UsingSingletonMut, UsingSingleton},
    thumbnail_cache::ThumbnailCache,
};

use anyhow::anyhow;

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

    pub fn thumbnail_path(&self) -> anyhow::Result<PathBuf> {
        let mut path = self.path.clone();
        let file_name = path
            .file_name()
            .ok_or(anyhow!("Failed to get file name"))?
            .to_str()
            .ok_or(anyhow!("Failed to convert file name to str"))?
            .to_string();
        path.pop();
        path.push(".thumb");
        path.push(file_name);
        Ok(path)
    }

    pub fn thumbnail<'a>(&self) -> Option<Vec<u8>> {
        Dependency::<ThumbnailCache>::using_singleton(|thumbnail_cache| thumbnail_cache.get(&self.path))
    }
}

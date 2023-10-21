use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use once_cell::sync::Lazy;

pub struct ThumbnailCache {
    cache: HashMap<String, Vec<u8>>,
}

impl ThumbnailCache {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    pub fn put(&mut self, path: &PathBuf, thumbnail: Vec<u8>) {
        self.cache
            .insert(path.to_str().unwrap().to_string(), thumbnail);
    }

    pub fn get(&self, path: &PathBuf) -> Option<Vec<u8>> {
        self.cache.get(path.to_str().unwrap()).cloned()
    }
}

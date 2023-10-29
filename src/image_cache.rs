use std::{collections::HashMap, path::PathBuf};

use eframe::egui::load::{SizedTexture, TextureLoadResult, TexturePoll};

pub struct ImageCache {
    cache: HashMap<String, TexturePoll>,
}

impl ImageCache {
    pub fn new() -> Self {
        Self {
            cache: HashMap::new(),
        }
    }

    pub fn put(&mut self, path: &PathBuf, image: TexturePoll) {
        println!("Putting image in cache: {:?}", path);
        self.cache.insert(path.display().to_string(), image);
    }

    pub fn get(&self, path: &PathBuf) -> Option<SizedTexture> {
        
        match self.cache.get(path.display().to_string().as_str()) {
            Some(texture_poll) => match texture_poll {
                TexturePoll::Pending { size } => None,
                TexturePoll::Ready { texture } => Some(texture.clone()),
            },
            None => None,
        }
    }
}

use std::path::PathBuf;

use crate::{
    dependencies::{Dependency, SendableSingletonFor, SingletonFor},
    thumbnail_cache::ThumbnailCache,
};

use anyhow::anyhow;
use eframe::epaint::Vec2;

pub enum PhotoDimension {
    Width(f32),
    Height(f32),
}

#[derive(Debug, Clone)]
pub struct Photo {
    pub path: PathBuf,
    pub size: Vec2,
}

impl Photo {
    pub fn new(path: PathBuf) -> Self {
        let size = imagesize::size(path.clone()).unwrap();

        Self {
            path,
            size: Vec2::new(size.width as f32, size.height as f32),
        }
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

    pub fn uri(&self) -> String {
        format!("file://{}", self.string_path())
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

    pub fn thumbnail<'a>(&self) -> anyhow::Result<Option<Vec<u8>>> {
        Dependency::<ThumbnailCache>::get()
            .with_lock(|thumbnail_cache| thumbnail_cache.get(&self.path))
    }

    pub fn max_dimension(&self) -> PhotoDimension {
        if self.size.x > self.size.y {
            PhotoDimension::Width(self.size.x)
        } else {
            PhotoDimension::Height(self.size.y)
        }
    }
}

use std::path::PathBuf;

use crate::{
    dependencies::{Dependency, SendableSingletonFor, SingletonFor},
    image_cache::ImageCache,
};

use anyhow::anyhow;
use eframe::{
    egui::{self, load::SizedTexture, Context, SizeHint, TextureOptions},
    epaint::{util::FloatOrd, Vec2},
};
use log::error;

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

    pub fn thumbnail_uri(&self) -> String {
        format!("file://{}", self.thumbnail_path().unwrap().display())
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

    // pub fn thumbnail(&self) -> anyhow::Result<Option<Vec<u8>>> {
    //     Dependency::<ImageCache>::get().with_lock(|image_cache| image_cache.get(&self.path))
    // }

    // pub fn thumbnail_texture(&self, ctx: &Context) -> anyhow::Result<Option<SizedTexture>> {
    //     match ctx.try_load_texture(
    //         &self.thumbnail_uri(),
    //         TextureOptions::LINEAR,
    //         SizeHint::Scale(1.0_f32.ord()),
    //     )? {
    //         egui::load::TexturePoll::Pending { size: _ } => Ok(None),
    //         egui::load::TexturePoll::Ready { texture } => Ok(Some(texture)),
    //     }
    // }

    // pub fn bytes(&self) -> anyhow::Result<Option<Vec<u8>>> {
    //     Dependency::<ImageCache>::get().with_lock_mut(|image_cache| {
    //         match image_cache.get(&self.path) {
    //             Some(bytes) => Some(bytes),
    //             None => match std::fs::read(&self.path).ok() {
    //                 Some(bytes) => {
    //                     image_cache.put(&self.path, bytes.clone());
    //                     Some(bytes)
    //                 }
    //                 None => None,
    //             },
    //         }
    //     })
    // }

    // pub fn texture(&self, ctx: &Context) -> anyhow::Result<Option<SizedTexture>> {
    //     match ctx.try_load_texture(
    //         &self.uri(),
    //         TextureOptions::LINEAR,
    //         SizeHint::Scale(1.0_f32.ord()),
    //     )? {
    //         egui::load::TexturePoll::Pending { size: _ } => Ok(None),
    //         egui::load::TexturePoll::Ready { texture } => Ok(Some(texture)),
    //     }
    // }

    pub fn max_dimension(&self) -> PhotoDimension {
        if self.size.x > self.size.y {
            PhotoDimension::Width(self.size.x)
        } else {
            PhotoDimension::Height(self.size.y)
        }
    }
}

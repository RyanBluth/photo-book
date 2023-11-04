use std::{
    collections::{HashMap, HashSet},
    fs::read_dir,
    path::PathBuf,
};

use eframe::{
    egui::{load::{SizedTexture, TextureLoader}, Context},
    epaint::util::FloatOrd,
};
use log::info;

use crate::{
    dependencies::{Dependency, DependencyFor},
    gallery_service::ThumbnailService,
    photo::Photo,
};

pub struct PhotoManager {
    pub photos: Vec<Photo>,

    texture_cache: HashMap<String, SizedTexture>,
    thumbnail_service: ThumbnailService,
    pending_textures: HashSet<String>,
}


impl PhotoManager {
    pub fn new() -> Self {
        Self {
            photos: Vec::new(),
            texture_cache: HashMap::new(),
            thumbnail_service: Dependency::<ThumbnailService>::get(),
            pending_textures: HashSet::new(),
        }
    }

    pub fn load_directory(&mut self, path: &PathBuf, ctx: &Context) -> anyhow::Result<()> {
        let entries: Vec<Result<std::fs::DirEntry, std::io::Error>> =
            read_dir(path).unwrap().collect();

        for entry in entries {
            let entry = entry.as_ref().unwrap();
            let path = entry.path();

            if path.extension().unwrap_or_default().to_ascii_lowercase() != "jpg" {
                continue;
            }
            self.photos.push(Photo::new(path));
        }

        self.thumbnail_service
            .gen_thumbnails(path.clone(), ctx.clone())
    }

    pub fn thumbnail_texture_for(
        &mut self,
        photo: &Photo,
        ctx: &Context,
    ) -> anyhow::Result<Option<SizedTexture>> {
        if !photo.thumbnail_path()?.exists() {
            return Ok(None);
        }

        Self::load_texture(
            &photo.thumbnail_uri(),
            &ctx,
            &mut self.texture_cache,
            &mut self.pending_textures,
        )
    }

    pub fn tumbnail_texture_at(
        &mut self,
        at: usize,
        ctx: &Context,
    ) -> anyhow::Result<Option<SizedTexture>> {
        match self.photos.get(at) {
            Some(photo) => {
                if !photo.thumbnail_path()?.exists() {
                    return Ok(None);
                }
                Self::load_texture(
                    &photo.thumbnail_uri(),
                    &ctx,
                    &mut self.texture_cache,
                    &mut self.pending_textures,
                )
            }
            None => Ok(None),
        }
    }

    pub fn texture_for(
        &mut self,
        photo: &Photo,
        ctx: &Context,
    ) -> anyhow::Result<Option<SizedTexture>> {
        Self::load_texture(
            &photo.uri(),
            &ctx,
            &mut self.texture_cache,
            &mut self.pending_textures,
        )
    }

    pub fn texture_at(&mut self, at: usize, ctx: &Context) -> anyhow::Result<Option<SizedTexture>> {
        match self.photos.get(at) {
            Some(photo) => Self::load_texture(
                &photo.uri(),
                &ctx,
                &mut self.texture_cache,
                &mut self.pending_textures,
            ),
            None => Ok(None),
        }
    }

    pub fn next_photo(
        &mut self,
        current_index: usize,
        ctx: &Context,
    ) -> anyhow::Result<Option<(Photo, usize)>> {
        let next_index = (current_index + 1) % self.photos.len();
        match self.photos.get(next_index) {
            Some(next_photo) => {
                if let Some(current_photo) = self.photos.get(current_index) {
                    if let Some(texture) = self.texture_cache.remove(&current_photo.uri()) {
                        info!("Freeing texture for photo {}", current_photo.uri());
                        ctx.forget_image(&current_photo.uri());
                        ctx.tex_manager().write().free(texture.id);
                    }
                }

                Ok(Some((next_photo.clone(), next_index)))
            }
            None => Ok(None),
        }
    }

    pub fn previous_photo(
        &mut self,
        current_index: usize,
        ctx: &Context,
    ) -> anyhow::Result<Option<(Photo, usize)>> {
        let prev_index = (current_index + self.photos.len() - 1) % self.photos.len();
        match self.photos.get(prev_index) {
            Some(previous_photo) => {
                if let Some(current_photo) = self.photos.get(current_index) {
                    if let Some(texture) = self.texture_cache.remove(&current_photo.uri()) {
                        info!("Freeing texture for photo {}", current_photo.uri());
                        ctx.forget_image(&current_photo.uri());
                        ctx.tex_manager().write().free(texture.id);
                    }
                }

                Ok(Some((previous_photo.clone(), prev_index)))
            }
            None => Ok(None),
        }
    }

    fn load_texture(
        uri: &str,
        ctx: &Context,
        texture_cache: &mut HashMap<String, SizedTexture>,
        pending_textures: &mut HashSet<String>,
    ) -> anyhow::Result<Option<SizedTexture>> {
        match texture_cache.get(uri) {
            Some(texture) => {
                pending_textures.remove(uri);
                Ok(Some(texture.clone()))
            }
            None => {
                let texture = ctx.try_load_texture(
                    uri,
                    eframe::egui::TextureOptions::default(),
                    eframe::egui::SizeHint::Scale(1.0_f32.ord()),
                )?;
                match texture {
                    eframe::egui::load::TexturePoll::Pending { size: _ } => {
                        pending_textures.insert(uri.to_string());
                        Ok(None)
                    }
                    eframe::egui::load::TexturePoll::Ready { texture } => {
                        texture_cache.insert(uri.to_string(), texture.clone());
                        Ok(Some(texture))
                    }
                }
            }
        }
    }
}

use std::{
    collections::{HashMap, HashSet},
    io::BufWriter,
    path::PathBuf,
};

use glob::MatchOptions;

use chrono::{DateTime, Datelike, Utc};
use eframe::egui::{load::SizedTexture, Context};
use egui::emath::OrderedFloat;
use fxhash::hash64;
use image::{
    codecs::{jpeg::JpegEncoder, png::PngEncoder},
    ExtendedColorType,
};
use indexmap::IndexMap;
use log::{error, info};
use tokio::task::spawn_blocking;
use tokio::{fs::File as TokioFile, io::AsyncWriteExt};

use crate::{
    dependencies::{Dependency, Singleton},
    dirs::Dirs,
    photo::{self, Photo, PhotoMetadataField, PhotoMetadataFieldLabel, PhotoRating},
};

use anyhow::{anyhow, Ok};
use fr::CpuExtensions;
use image::io::Reader as ImageReader;
use image::ImageEncoder;

use core::result::Result;

use fast_image_resize::{self as fr, ResizeOptions};

use crate::dependencies::SingletonFor;

use crate::utils;

const THUMBNAIL_SIZE: f32 = 256.0;

#[derive(Clone, Debug)]
pub enum PhotoLoadResult {
    Pending(PathBuf),
    Ready(Photo),
}

impl PhotoLoadResult {
    pub fn path(&self) -> &PathBuf {
        match self {
            PhotoLoadResult::Pending(path) => path,
            PhotoLoadResult::Ready(photo) => &photo.path,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Copy)]
pub enum PhotosGrouping {
    Date,
    Rating,
}

impl Default for PhotosGrouping {
    fn default() -> Self {
        Self::Date
    }
}

#[derive(Debug)]
pub struct PhotoManager {
    pub photos: IndexMap<PathBuf, Photo>, // TODO: Use an Arc or something
    grouped_photos: (PhotosGrouping, IndexMap<String, IndexMap<PathBuf, Photo>>), // TODO: Use an Arc or something
    texture_cache: HashMap<String, SizedTexture>,
    pending_textures: HashSet<String>,
    thumbnail_existence_cache: HashSet<String>,
}

impl PhotoManager {
    pub fn new() -> Self {
        Self {
            photos: IndexMap::new(),
            grouped_photos: (PhotosGrouping::default(), IndexMap::new()),
            texture_cache: HashMap::new(),
            pending_textures: HashSet::new(),
            thumbnail_existence_cache: HashSet::new(),
        }
    }

    fn photo_exists(&self, path: &PathBuf) -> bool {
        self.photos.contains_key(path)
    }

    pub fn load_directory(path: PathBuf) -> anyhow::Result<()> {
        tokio::spawn(async move {
            let glob_patterns = vec![
                format!("{}/**/*.jpg", path.to_string_lossy()),
                format!("{}/**/*.jpeg", path.to_string_lossy()),
            ];

            let glob_iter = glob_patterns.iter().flat_map(|pattern: &String| {
                glob::glob_with(
                    pattern,
                    MatchOptions {
                        case_sensitive: false,
                        require_literal_separator: false,
                        require_literal_leading_dot: false,
                    },
                )
                .unwrap()
            });

            let pending_photos: Vec<PathBuf> = glob_iter
                .filter_map(|entry| {
                    let path = entry.as_ref().ok()?;
                    let lowercase_extension = path.extension()?.to_ascii_lowercase();
                    if (lowercase_extension == "jpg" || lowercase_extension == "jpeg") 
                        && !Dependency::<PhotoManager>::get().with_lock(|pm| pm.photo_exists(path)) {
                        Some(path.clone())
                    } else {
                        None
                    }
                })
                .collect();

            for photo_path in pending_photos {
                match Photo::new_async(photo_path.clone()).await {
                    Result::Ok(photo) => {
                        Dependency::<PhotoManager>::get().with_lock_mut(|photo_manager| {
                            photo_manager.photos.insert(photo_path.clone(), photo);
                        });
                    }
                    Err(err) => {
                        error!("Failed to load photo: {:?} - {:?}", photo_path, err);
                    }
                }
            }

            Dependency::<PhotoManager>::get().with_lock_mut(|photo_manager| {
                photo_manager.photos.sort_by(|_, a, _, b| {
                    match (
                        a.metadata.fields.get(PhotoMetadataFieldLabel::DateTime),
                        b.metadata.fields.get(PhotoMetadataFieldLabel::DateTime),
                    ) {
                        (
                            Some(PhotoMetadataField::DateTime(a)),
                            Some(PhotoMetadataField::DateTime(b)),
                        ) => b.cmp(a),
                        _ => b.path.cmp(&a.path),
                    }
                });

                photo_manager.regroup_photos();
            });

            let photo_paths: Vec<PathBuf> =
                Dependency::<PhotoManager>::get().with_lock(|photo_manager| {
                    photo_manager
                        .photos
                        .keys()
                        .cloned()
                        .collect::<Vec<PathBuf>>()
                });

            let _ = Self::gen_thumbnails(photo_paths);

            Ok(())
        });

        Ok(())
    }

    pub fn load_photos(&self, photos: Vec<(PathBuf, Option<PhotoRating>)>) {
        tokio::spawn(async move {
            let mut photos_since_regroup: usize = 0;
            let filtered_photos: Vec<(PathBuf, Option<PhotoRating>)> = photos
                .into_iter()
                .filter(|(path, _)| !Dependency::<PhotoManager>::get().with_lock(|pm| pm.photo_exists(path)))
                .collect();

            let num_photos = filtered_photos.len();

            for (path, rating) in filtered_photos {
                let photo =
                    Photo::with_rating_async(path.clone(), rating.unwrap_or_default()).await;

                match photo {
                    Result::Err(err) => {
                        error!("Failed to load photo: {:?} - {:?}", path, err);
                        continue;
                    }
                    Result::Ok(photo) => {
                        Dependency::<PhotoManager>::get().with_lock_mut(|photo_manager| {
                            photo_manager.photos.insert(path.clone(), photo);

                            photos_since_regroup += 1;

                            if photos_since_regroup > 500 || num_photos == photos_since_regroup {
                                photos_since_regroup = 0;
                                photo_manager.sort_and_regroup();
                            }
                        });
                    }
                };
            }

            let (photo_paths, _) =
                Dependency::<PhotoManager>::get().with_lock_mut(|photo_manager| {
                    photo_manager.regroup_photos();

                    let photo_paths: Vec<PathBuf> = photo_manager.photos.keys().cloned().collect();
                    let thumbnail_dir = Dirs::Thumbnails.path();

                    (photo_paths, thumbnail_dir)
                });
            let _ = Self::gen_thumbnails(photo_paths);
        });
    }

    // Add helper method for sorting and regrouping
    fn sort_and_regroup(&mut self) {
        self.photos.sort_by(|_, a, _, b| {
            match (
                a.metadata.fields.get(PhotoMetadataFieldLabel::DateTime),
                b.metadata.fields.get(PhotoMetadataFieldLabel::DateTime),
            ) {
                (Some(PhotoMetadataField::DateTime(a)), Some(PhotoMetadataField::DateTime(b))) => {
                    b.cmp(a)
                }
                _ => b.path.cmp(&a.path),
            }
        });

        self.regroup_photos();
    }

    pub fn grouped_photos(&self) -> &IndexMap<String, IndexMap<PathBuf, Photo>> {
        &self.grouped_photos.1
    }

    fn regroup_photos(&mut self) {
        let grouping = self.grouped_photos.0;
        self.group_photos_by(grouping);
    }

    pub fn photo_grouping(&self) -> PhotosGrouping {
        self.grouped_photos.0
    }

    pub fn group_photos_by(
        &mut self,
        photos_grouping: PhotosGrouping,
    ) -> &IndexMap<String, IndexMap<PathBuf, Photo>> {
        let photos = &self.photos;
        match photos_grouping {
            PhotosGrouping::Date => {
                let mut grouped_photos: IndexMap<String, IndexMap<PathBuf, Photo>> =
                    IndexMap::new();

                for (photo_path, photo) in photos.iter() {
                    let exif_date_time = photo
                        .metadata
                        .fields
                        .get(PhotoMetadataFieldLabel::DateTime)
                        .and_then(|field| match field {
                            PhotoMetadataField::DateTime(date_time) => Some(date_time.clone()),
                            _ => {
                                if let Result::Ok(file_metadata) = std::fs::metadata(photo_path) {
                                    if let Result::Ok(modified) = file_metadata.modified() {
                                        let modified_date_time: DateTime<Utc> = modified.into();
                                        return Some(modified_date_time);
                                    } else if let Result::Ok(created) = file_metadata.created() {
                                        let created_date_time: DateTime<Utc> = created.into();
                                        return Some(created_date_time);
                                    } else {
                                        return None;
                                    }
                                } else {
                                    return None;
                                }
                            }
                        });

                    let key = if let Some(date_time) = exif_date_time {
                        let year = date_time.year();
                        let month = date_time.month();
                        let day = date_time.day();

                        Some(format!("{:04}-{:02}-{:02}", year, month, day))
                    } else {
                        // Get the last modified date of the file
                        let metadata = std::fs::metadata(photo_path).unwrap();
                        let modified = metadata.modified().unwrap();
                        let modified_date_time: DateTime<Utc> = modified.into();
                        let year = modified_date_time.year();
                        let month: u32 = modified_date_time.month();
                        let day = modified_date_time.day();

                        Some(format!("{:04}-{:02}-{:02}", year, month, day))
                    }
                    .unwrap_or_else(|| "Unknown Date".to_string());

                    if let Some(group) = grouped_photos.get_mut(&key) {
                        group.insert(photo_path.clone(), photo.clone());
                    } else {
                        let mut group = IndexMap::new();
                        group.insert(photo_path.clone(), photo.clone());
                        grouped_photos.insert(key, group);
                    }
                }

                grouped_photos.sort_by(|a, _, b, _| b.cmp(a));

                self.grouped_photos = (PhotosGrouping::Date, grouped_photos);
            }
            PhotosGrouping::Rating => {
                let mut grouped_photos: IndexMap<String, IndexMap<PathBuf, Photo>> =
                    IndexMap::new();

                for (photo_path, photo) in photos.iter() {
                    let rating = photo.rating;
                    let key = format!("{:?}", rating);

                    if let Some(group) = grouped_photos.get_mut(&key) {
                        group.insert(photo_path.clone(), photo.clone());
                    } else {
                        let mut group = IndexMap::new();
                        group.insert(photo_path.clone(), photo.clone());
                        grouped_photos.insert(key, group);
                    }
                }

                if let Some(yes_index) = grouped_photos.get_index_of(&PhotoRating::Yes.to_string())
                {
                    grouped_photos.move_index(yes_index, 0);
                }

                if let Some(no_index) = grouped_photos.get_index_of(&PhotoRating::No.to_string()) {
                    grouped_photos.move_index(no_index, grouped_photos.len() - 1);
                }

                self.grouped_photos = (PhotosGrouping::Rating, grouped_photos);
            }
        }

        &self.grouped_photos.1
    }

    pub fn update_photo(&mut self, photo: Photo) {
        self.photos.insert(photo.path.clone(), photo.clone());
        for group in self.grouped_photos.1.values_mut() {
            if group.contains_key(&photo.path) {
                group.insert(photo.path.clone(), photo.clone());
                self.regroup_photos(); // TODO: This isn't very efficient
                return;
            }
        }
    }

    pub fn thumbnail_texture_for(
        &mut self,
        photo: &Photo,
        ctx: &Context,
    ) -> anyhow::Result<Option<SizedTexture>> {
        if !self
            .thumbnail_existence_cache
            .contains(&photo.thumbnail_hash)
        {
            return Ok(None);
        }

        Self::load_texture(
            &photo.thumbnail_uri(),
            ctx,
            &mut self.texture_cache,
            &mut self.pending_textures,
        )
    }

    pub fn tumbnail_texture_at(
        &mut self,
        at: usize,
        ctx: &Context,
    ) -> anyhow::Result<Option<SizedTexture>> {
        match self.photos.get_index(at) {
            Some((_, photo)) => {
                if !self
                    .thumbnail_existence_cache
                    .contains(&photo.thumbnail_hash)
                {
                    return Ok(None);
                }
                Self::load_texture(
                    &photo.thumbnail_uri(),
                    ctx,
                    &mut self.texture_cache,
                    &mut self.pending_textures,
                )
            }
            _ => Ok(None),
        }
    }

    pub fn texture_for(
        &mut self,
        photo: &Photo,
        ctx: &Context,
    ) -> anyhow::Result<Option<SizedTexture>> {
        Self::load_texture(
            &photo.uri(),
            ctx,
            &mut self.texture_cache,
            &mut self.pending_textures,
        )
    }

    pub fn texture_for_blocking(
        &mut self,
        photo: &Photo,
        ctx: &Context,
    ) -> anyhow::Result<Option<SizedTexture>> {
        Self::load_texture_blocking(
            &photo.uri(),
            ctx,
            &mut self.texture_cache,
            &mut self.pending_textures,
        )
    }

    pub fn texture_for_photo_with_thumbail_backup(
        &mut self,
        photo: &Photo,
        ctx: &Context,
    ) -> anyhow::Result<Option<SizedTexture>> {
        match Self::load_texture(
            &photo.uri(),
            ctx,
            &mut self.texture_cache,
            &mut self.pending_textures,
        ) {
            Result::Ok(Some(tex)) => Ok(Some(tex)),
            _ => Ok(self.texture_cache.get(&photo.thumbnail_uri()).copied()),
        }
    }

    pub fn texture_at(&mut self, at: usize, ctx: &Context) -> anyhow::Result<Option<SizedTexture>> {
        match self.photos.get_index(at) {
            Some((_, photo)) => Self::load_texture(
                &photo.uri(),
                ctx,
                &mut self.texture_cache,
                &mut self.pending_textures,
            ),
            _ => Ok(None),
        }
    }

    pub fn next_photo(
        &mut self,
        current_photo: &Photo,
        _ctx: &Context,
    ) -> anyhow::Result<Option<(Photo, usize)>> {
        let current_index = self
            .index_for_photo(current_photo)
            .ok_or(anyhow!("Photo not found"))?;
        let next_index = (current_index + 1) % self.photos.len();
        match self.photos.get_index(next_index) {
            Some((_, next_photo)) => {
                if let Some((_, current_photo)) = self.photos.get_index(current_index) {
                    if let Some(_texture) = self.texture_cache.remove(&current_photo.uri()) {
                        // info!("Freeing texture for photo {}", current_photo.uri());
                        // ctx.forget_image(&current_photo.uri());
                        // ctx.tex_manager().write().free(texture.id);
                    }
                }

                Ok(Some((next_photo.clone(), next_index)))
            }
            _ => Ok(None),
        }
    }

    pub fn previous_photo(
        &mut self,
        current_photo: &Photo,
        _ctx: &Context,
    ) -> anyhow::Result<Option<(Photo, usize)>> {
        let current_index = self
            .index_for_photo(current_photo)
            .ok_or(anyhow!("Photo not found"))?;
        let prev_index = (current_index + self.photos.len() - 1) % self.photos.len();
        match self.photos.get_index(prev_index) {
            Some((_, previous_photo)) => {
                if let Some((_, current_photo)) = self.photos.get_index(current_index) {
                    if let Some(_texture) = self.texture_cache.remove(&current_photo.uri()) {
                        // info!("Freeing texture for photo {}", current_photo.uri());
                        // ctx.forget_image(&current_photo.uri());
                        // ctx.tex_manager().write().free(texture.id);
                    }
                }

                Ok(Some((previous_photo.clone(), prev_index)))
            }
            _ => Ok(None),
        }
    }

    fn index_for_photo(&self, photo: &Photo) -> Option<usize> {
        self.photos.get_full(&photo.path).map(|(index, _, _)| index)
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
                Ok(Some(*texture))
            }
            None => {
                let uri = uri.to_string();
                let ctx = ctx.clone();
                spawn_blocking(move || {
                    let texture: Result<egui::load::TexturePoll, egui::load::LoadError> = ctx.try_load_texture(
                        &uri,
                        eframe::egui::TextureOptions::default(),
                        eframe::egui::SizeHint::Scale(OrderedFloat(1.0)),
                    );

                    let photo_manager = Dependency::<PhotoManager>::get();
                    match texture {
                        Result::Ok(eframe::egui::load::TexturePoll::Pending { size: _ }) => {
                            photo_manager.with_lock_mut(|photo_manager| {
                                photo_manager.pending_textures.insert(uri)
                            });
                        }
                        Result::Ok(eframe::egui::load::TexturePoll::Ready { texture }) => {
                            photo_manager.with_lock_mut(|photo_manager| {
                                photo_manager.texture_cache.insert(uri, texture);
                            });
                        }
                        Result::Err(err) => {
                            error!("Failed to load texture {:?}", err);
                        }
                    }
                });

                Ok(None)
            }
        }
    }

    fn load_texture_blocking(
        uri: &str,
        ctx: &Context,
        texture_cache: &mut HashMap<String, SizedTexture>,
        pending_textures: &mut HashSet<String>,
    ) -> anyhow::Result<Option<SizedTexture>> {
        match texture_cache.get(uri) {
            Some(texture) => {
                pending_textures.remove(uri);
                Ok(Some(*texture))
            }
            None => {
                let texture = ctx.try_load_texture(
                    uri,
                    eframe::egui::TextureOptions::default(),
                    eframe::egui::SizeHint::Scale(OrderedFloat::from(1.0)),
                );

                match texture {
                    Result::Ok(eframe::egui::load::TexturePoll::Pending { size: _ }) => {
                        pending_textures.insert(uri.to_string());
                        Ok(None)
                    }
                    Result::Ok(eframe::egui::load::TexturePoll::Ready { texture }) => {
                        texture_cache.insert(uri.to_string(), texture);
                        Ok(Some(texture))
                    }
                    Result::Err(err) => Err(anyhow!(err)),
                }
            }
        }
    }

    fn gen_thumbnails(photo_paths: Vec<PathBuf>) -> anyhow::Result<()> {
        let thumbnail_dir = Dirs::Thumbnails.path();

        let partitions = utils::partition_iterator(photo_paths.into_iter(), 16);

        for partition in partitions {
            let thumbnail_dir: PathBuf = thumbnail_dir.clone();
            tokio::task::spawn(async move {
                for photo in partition {
                    let res: Result<(), anyhow::Error> =
                        Self::gen_thumbnail(&photo, &thumbnail_dir).await;
                    if res.is_err() {
                        // TODO: Handle this better
                        error!("{:?}", res);
                        // panic!("{:?}", res);
                    }
                }
            });
        }
        Ok(())
    }

    async fn gen_thumbnail(photo_path: &PathBuf, thumbnail_dir: &PathBuf) -> anyhow::Result<()> {
        let file_name = photo_path.file_name();
        let extension = photo_path.extension();

        if let (Some(_), Some(extension)) = (file_name, extension) {
            if extension.to_ascii_lowercase() == "jpg"
                || extension.to_ascii_lowercase() == "png"
                || extension.to_ascii_lowercase() == "jpeg"
            {
                // TODO: incorporate the last modified date of the photo into the hash
                let hash = hash64(&photo_path.to_string_lossy()).to_string();

                let mut thumbnail_path = thumbnail_dir.join(&hash);
                thumbnail_path.set_extension(extension);

                if thumbnail_path.exists() {
                    info!("Thumbnail already exists for: {:?}", &photo_path);
                    Dependency::<PhotoManager>::get().with_lock_mut(|photo_manager| {
                        photo_manager.thumbnail_existence_cache.insert(hash);
                    });

                    return Ok(());
                } else {
                    info!("Generating thumbnail: {:?}", &thumbnail_path);
                }

                let file_bytes = tokio::fs::read(photo_path).await?;
                let img = spawn_blocking(move || {
                    image::ImageReader::new(std::io::Cursor::new(file_bytes))
                        .with_guessed_format()?
                        .decode()
                })
                .await??;

                let color_type = img.color();

                let width = img.width();
                let height = img.height();

                let mut src_image = fr::images::Image::from_vec_u8(
                    img.width(),
                    img.height(),
                    // TODO: This isn't going to cover every type of image
                    if color_type.has_alpha() {
                        img.to_rgba8().into_raw()
                    } else {
                        img.into_rgb8().into_raw()
                    },
                    if color_type.has_alpha() {
                        fr::PixelType::U8x4
                    } else {
                        fr::PixelType::U8x3
                    },
                )?;

                // Multiple RGB channels of source image by alpha channel
                // (not required for the Nearest algorithm)
                let alpha_mul_div = fr::MulDiv::default();

                if color_type.has_alpha() {
                    alpha_mul_div.multiply_alpha_inplace(&mut src_image)?;
                }

                let ratio = height as f32 / width as f32;
                let dst_height: u32 = (THUMBNAIL_SIZE * ratio) as u32;
                let dst_width: u32 = THUMBNAIL_SIZE as u32;

                let (dst_width, dst_height) = (dst_width, dst_height);
                let pixel_type = src_image.pixel_type();
                let src_image = src_image;
                let color_type = color_type;

                let dst_image = spawn_blocking(move || {
                    let mut dst_image = fr::images::Image::new(dst_width, dst_height, pixel_type);
                    let mut resizer = fr::Resizer::new();

                    // CPU extensions setup
                    let mut cpu_extensions_vec = vec![CpuExtensions::None];
                    #[cfg(target_arch = "x86_64")]
                    {
                        cpu_extensions_vec.push(CpuExtensions::Sse4_1);
                        cpu_extensions_vec.push(CpuExtensions::Avx2);
                    }
                    #[cfg(target_arch = "aarch64")]
                    {
                        cpu_extensions_vec.push(CpuExtensions::Neon);
                    }
                    #[cfg(target_arch = "wasm32")]
                    {
                        cpu_extensions_vec.push(CpuExtensions::Simd128);
                    }

                    for cpu_extension in cpu_extensions_vec {
                        if cpu_extension.is_supported() {
                            unsafe {
                                resizer.set_cpu_extensions(cpu_extension);
                                break;
                            }
                        }
                    }

                    resizer.resize(
                        &src_image,
                        &mut dst_image,
                        &ResizeOptions {
                            algorithm: fast_image_resize::ResizeAlg::Nearest,
                            cropping: fast_image_resize::SrcCropping::None,
                            mul_div_alpha: false,
                        },
                    )?;

                    if color_type.has_alpha() {
                        let alpha_mul_div = fr::MulDiv::default();
                        alpha_mul_div.divide_alpha_inplace(&mut dst_image)?;
                    }

                    Ok(dst_image)
                })
                .await??;

                // Write destination image as PNG-file
                let mut result_buf = BufWriter::new(Vec::new());

                match extension
                    .to_ascii_lowercase()
                    .to_str()
                    .ok_or(anyhow!("Failed to convert extension to str"))?
                {
                    "jpg" | "jpeg" => {
                        JpegEncoder::new_with_quality(&mut result_buf, 60).write_image(
                            dst_image.buffer(),
                            dst_width,
                            dst_height,
                            ExtendedColorType::Rgb8,
                        )?;
                    }
                    "png" => {
                        PngEncoder::new(&mut result_buf).write_image(
                            dst_image.buffer(),
                            dst_width,
                            dst_height,
                            ExtendedColorType::Rgba8,
                        )?;
                    }
                    _ => {
                        return Err(anyhow::anyhow!("Invalid file extension"));
                    }
                }

                let buf = result_buf.into_inner()?;

                let mut file = TokioFile::create(&thumbnail_path).await?;
                file.write_all(&buf).await?;
                file.sync_all().await?;

                info!("Thumbnail generated: {:?}", &thumbnail_path);

                Dependency::<PhotoManager>::get().with_lock_mut(|photo_manager| {
                    photo_manager.thumbnail_existence_cache.insert(hash);
                });

                //ctx.request_repaint();
            }
        }

        Ok(())
    }
}

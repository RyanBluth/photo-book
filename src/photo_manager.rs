use std::{
    collections::{HashMap, HashSet},
    io::BufWriter,
    path::PathBuf,
};

use glob::MatchOptions;

use chrono::{DateTime, Utc};
use eframe::egui::{Context, load::SizedTexture};
use egui::emath::OrderedFloat;
use fxhash::hash64;
use image::{
    ExtendedColorType,
    codecs::{jpeg::JpegEncoder, png::PngEncoder},
};
use indexmap::IndexMap;
use log::{error, info};
use tokio::task::spawn_blocking;
use tokio::{fs::File as TokioFile, io::AsyncWriteExt};

use crate::{
    dependencies::Dependency,
    dirs::Dirs,
    model::photo_grouping::PhotoGrouping,
    photo::{Photo, PhotoRating},
    photo_database::{
        PhotoDatabase, PhotoQuery, PhotoQueryResult, PhotoQueryResultIterator, PhotoSortCriteria,
    },
};

use anyhow::{Ok, anyhow};
use fr::CpuExtensions;
use image::ImageEncoder;

use core::result::Result;

use fast_image_resize::{self as fr, ResizeOptions};

use crate::dependencies::SingletonFor;

use crate::utils;

const THUMBNAIL_SIZE: f32 = 512.0;

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

pub struct PhotoMetadata {
    pub rating: Option<PhotoRating>,
    pub date_time: Option<DateTime<Utc>>,
    pub index: usize,
    pub grouped_index: usize,
}

#[derive(Debug)]
pub struct PhotoManager {
    current_grouping: PhotoGrouping,
    current_filter: PhotoQuery,
    texture_cache: HashMap<String, SizedTexture>,
    pending_textures: HashSet<String>,
    thumbnail_existence_cache: HashSet<String>,
    current_query_result: Option<PhotoQueryResult>,
    pub photo_database: PhotoDatabase,
}

impl PhotoManager {
    pub fn new() -> Self {
        Self {
            current_grouping: PhotoGrouping::default(),
            current_filter: PhotoQuery::default(),
            texture_cache: HashMap::new(),
            pending_textures: HashSet::new(),
            thumbnail_existence_cache: HashSet::new(),
            current_query_result: None,
            photo_database: PhotoDatabase::new(),
        }
    }

    fn photo_exists(&self, path: &PathBuf) -> bool {
        self.photo_database.photo_exists(path)
    }

    pub fn has_photos(&self) -> bool {
        self.photo_database.has_photos()
    }

    pub fn clear(&mut self) {
        self.current_grouping = PhotoGrouping::default();
        self.current_filter = PhotoQuery::default();
        self.texture_cache.clear();
        self.pending_textures.clear();
        self.thumbnail_existence_cache.clear();
        self.current_query_result = None;
        self.photo_database.clear();
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
                        && !Dependency::<PhotoManager>::get().with_lock(|pm| pm.photo_exists(path))
                    {
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
                            photo_manager.photo_database.add_photo(photo);
                        });
                    }
                    Err(err) => {
                        error!("Failed to load photo: {:?} - {:?}", photo_path, err);
                    }
                }
            }

            Dependency::<PhotoManager>::get().with_lock_mut(|photo_manager| {
                photo_manager
                    .photo_database
                    .sort_photos(PhotoSortCriteria::Date);
            });

            let photo_paths: Vec<PathBuf> = Dependency::<PhotoManager>::get()
                .with_lock(|photo_manager| photo_manager.photo_database.get_all_photo_paths());

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
                .filter(|(path, _)| {
                    !Dependency::<PhotoManager>::get().with_lock(|pm| pm.photo_exists(path))
                })
                .collect();

            let num_photos = filtered_photos.len();

            for (path, _) in filtered_photos {
                let photo = Photo::new_async(path.clone()).await;

                match photo {
                    Result::Err(err) => {
                        error!("Failed to load photo: {:?} - {:?}", path, err);
                        continue;
                    }
                    Result::Ok(photo) => {
                        Dependency::<PhotoManager>::get().with_lock_mut(|photo_manager| {
                            photo_manager.photo_database.add_photo(photo);

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
                    let photo_paths: Vec<PathBuf> =
                        photo_manager.photo_database.get_all_photo_paths();
                    let thumbnail_dir = Dirs::Thumbnails.path();

                    (photo_paths, thumbnail_dir)
                });
            let _ = Self::gen_thumbnails(photo_paths);
        });
    }

    // Add helper method for sorting and regrouping
    fn sort_and_regroup(&mut self) {
        self.photo_database.sort_photos(PhotoSortCriteria::Date);
    }

    pub fn grouped_photos(&mut self) -> IndexMap<String, IndexMap<PathBuf, Photo>> {
        let mut query = self.current_filter.clone();

        // Ensure grouping is set
        query.grouping = self.current_grouping;
        let query_result = self.photo_database.query_photos(&query);

        if let Some(current_query_result) = &self.current_query_result
            && query_result.id() == current_query_result.id()
        {
            current_query_result.groups.clone()
        } else {
            let groups = query_result.groups.clone();
            self.current_query_result = Some(query_result);
            groups
        }
    }

    pub fn photo_grouping(&self) -> PhotoGrouping {
        self.current_grouping
    }

    /// Change the photo grouping to given one
    pub fn group_photos_by(
        &mut self,
        photos_grouping: PhotoGrouping,
    ) -> IndexMap<String, IndexMap<PathBuf, Photo>> {
        self.current_grouping = photos_grouping;
        self.grouped_photos()
    }

    pub fn update_photo(&mut self, photo: Photo) {
        // Update photo in database if it exists, otherwise add it
        if self.photo_database.get_photo(&photo.path).is_some() {
            self.photo_database.update_photo(photo);
        } else {
            self.photo_database.add_photo(photo);
        }
        // PhotoDatabase handles invalidation of query cache automatically
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
        match self.photo_database.get_photo_by_index(at) {
            Some(photo) => {
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
        match self.photo_database.get_photo_by_index(at) {
            Some(photo) => Self::load_texture(
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
    ) -> anyhow::Result<Option<Photo>> {
        match &self.current_query_result {
            Some(query_result) => Ok(query_result.photo_after(current_photo)),
            None => Ok(None),
        }
    }

    pub fn previous_photo(
        &mut self,
        current_photo: &Photo,
        _ctx: &Context,
    ) -> anyhow::Result<Option<Photo>> {
        match &self.current_query_result {
            Some(query_result) => Ok(query_result.photo_before(current_photo)),
            None => Ok(None),
        }
    }

    fn index_for_photo(&mut self, photo: &Photo) -> Option<usize> {
        self.photo_database.get_photo_index(&photo.path)
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
                    let texture: Result<egui::load::TexturePoll, egui::load::LoadError> = ctx
                        .try_load_texture(
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
                    // info!("Thumbnail already exists for: {:?}", &photo_path);
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

    pub fn all_tags(&self) -> Vec<String> {
        self.photo_database.all_tags()
    }

    pub fn get_photo_rating(&self, path: &PathBuf) -> PhotoRating {
        self.photo_database.get_photo_rating(path)
    }

    pub fn set_photo_rating(&mut self, path: &PathBuf, rating: PhotoRating) {
        self.photo_database.set_photo_rating(path, rating);
        // PhotoDatabase handles invalidation of query cache automatically
    }

    pub fn get_photo_tags(&self, path: &PathBuf) -> HashSet<String> {
        self.photo_database.get_photo_tags(path)
    }

    pub fn set_photo_tags(&mut self, path: &PathBuf, tags: HashSet<String>) {
        self.photo_database.set_photo_tags(path, tags);
    }

    pub fn add_photo_tag(&mut self, path: &PathBuf, tag: String) {
        self.photo_database.add_photo_tag(path, tag);
    }

    pub fn remove_photo_tag(&mut self, path: &PathBuf, tag: &String) {
        self.photo_database.remove_photo_tag(path, tag);
    }

    pub fn get_current_filter(&self) -> &PhotoQuery {
        &self.current_filter
    }

    pub fn set_current_filter(&mut self, filter: PhotoQuery) {
        self.current_grouping = filter.grouping;
        self.current_filter = filter;
    }

    pub fn clear_current_filter(&mut self) {
        self.current_filter = PhotoQuery::default();
    }
}

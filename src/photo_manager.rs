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
    dep, dep_mut,
    dirs::Dirs,
    model::{
        album::{Album, AlbumId},
        photo_grouping::PhotoGrouping,
    },
    photo::{Photo, PhotoRating},
    photo_database::{PhotoDatabase, PhotoQuery, PhotoQueryResult, PhotoSortCriteria},
    photo_io,
};

use anyhow::{Ok, anyhow};
use fr::CpuExtensions;
use image::ImageEncoder;

use core::result::Result;

use fast_image_resize::{self as fr, ResizeOptions};

use crate::utils;

const THUMBNAIL_SIZE: f32 = 512.0;
const FULL_RES_PRELOAD_COUNT: usize = 4;
const FULL_RES_PRELOAD_START_BUDGET: usize = 4;
const FULL_RES_PRELOAD_READY_BUDGET: usize = 2;
const FULL_RES_CACHE_CAPACITY: usize = 20;

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub enum PhotoLoadResult {
    Pending(PathBuf),
    Ready(Photo),
}

impl PhotoLoadResult {
    #[allow(dead_code)]
    pub fn path(&self) -> &PathBuf {
        match self {
            PhotoLoadResult::Pending(path) => path,
            PhotoLoadResult::Ready(photo) => &photo.path,
        }
    }
}

#[allow(dead_code)]
pub struct PhotoMetadata {
    pub rating: Option<PhotoRating>,
    pub date_time: Option<DateTime<Utc>>,
    pub index: usize,
    pub grouped_index: usize,
}

#[derive(Debug, Default)]
struct ContextCache {
    texture_cache: HashMap<String, SizedTexture>,
    pending_textures: HashSet<String>,
    full_res_accesses: HashMap<String, u64>,
    full_res_access_counter: u64,
}

#[derive(Debug)]
pub struct PhotoManager {
    current_grouping: PhotoGrouping,
    current_filter: PhotoQuery,
    caches: Vec<(Context, ContextCache)>,
    thumbnail_existence_cache: HashSet<String>,
    rendered_existence_cache: HashSet<String>,
    pending_rendered_images: HashSet<String>,
    current_query_result: Option<PhotoQueryResult>,
    pub photo_database: PhotoDatabase,
}

impl PhotoManager {
    pub fn new() -> Self {
        Self {
            current_grouping: PhotoGrouping::default(),
            current_filter: PhotoQuery::default(),
            caches: Vec::new(),
            thumbnail_existence_cache: HashSet::new(),
            rendered_existence_cache: HashSet::new(),
            pending_rendered_images: HashSet::new(),
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
        self.caches.clear();
        self.thumbnail_existence_cache.clear();
        self.rendered_existence_cache.clear();
        self.pending_rendered_images.clear();
        self.current_query_result = None;
        self.photo_database.clear();
    }

    pub fn remove_cached_textures_for_context(&mut self, ctx: &Context) {
        self.caches.retain(|(context, _)| context != ctx);
    }

    fn get_cache_mut(&mut self, ctx: &Context) -> &mut ContextCache {
        let index = self.caches.iter().position(|(c, _)| c == ctx);
        match index {
            Some(i) => &mut self.caches[i].1,
            None => {
                self.caches.push((ctx.clone(), ContextCache::default()));
                &mut self.caches.last_mut().unwrap().1
            }
        }
    }

    pub fn load_directory(path: PathBuf, ctx: Context) -> anyhow::Result<()> {
        tokio::spawn(async move {
            let glob_patterns: Vec<String> = photo_io::SUPPORTED_PHOTO_EXTENSIONS
                .iter()
                .map(|extension| format!("{}/**/*.{}", path.to_string_lossy(), extension))
                .collect();

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
                    if photo_io::is_supported_photo_path(path)
                        && !dep!(PhotoManager, |pm| pm.photo_exists(path))
                    {
                        Some(path.clone())
                    } else {
                        None
                    }
                })
                .collect();

            let (standard_pending_photos, raw_pending_photos): (Vec<PathBuf>, Vec<PathBuf>) =
                pending_photos
                    .into_iter()
                    .partition(|path| !photo_io::is_raw_photo_path(path));

            let mut discovered_photo_paths = standard_pending_photos.clone();
            discovered_photo_paths.extend(raw_pending_photos.iter().cloned());
            Self::add_discovered_photo_paths(&discovered_photo_paths);
            ctx.request_repaint();

            let _ =
                Self::gen_thumbnail_batch(standard_pending_photos.clone(), Dirs::Thumbnails.path())
                    .await;
            Self::resolve_photo_metadata(standard_pending_photos).await;
            ctx.request_repaint();

            let _ = Self::gen_thumbnail_batch(raw_pending_photos.clone(), Dirs::Thumbnails.path())
                .await;
            Self::resolve_photo_metadata(raw_pending_photos).await;
            ctx.request_repaint();

            Ok(())
        });

        Ok(())
    }

    pub fn load_photos(&self, photos: Vec<(PathBuf, Option<PhotoRating>)>) {
        tokio::spawn(async move {
            let filtered_photo_paths: Vec<PathBuf> = photos
                .into_iter()
                .map(|(path, _)| path)
                .filter(|path| path.exists())
                .filter(|path| photo_io::is_supported_photo_path(path))
                .filter(|path| !dep!(PhotoManager, |pm| pm.photo_exists(path)))
                .collect();

            Self::add_discovered_photo_paths(&filtered_photo_paths);

            let photo_paths: Vec<PathBuf> = dep!(PhotoManager, |photo_manager| photo_manager
                .photo_database
                .get_all_photo_paths());
            let _ = Self::gen_thumbnails(photo_paths).await;
            Self::resolve_photo_metadata(filtered_photo_paths).await;
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

        let cache = self.get_cache_mut(ctx);
        Self::load_texture(
            &photo.thumbnail_uri(),
            ctx,
            &mut cache.texture_cache,
            &mut cache.pending_textures,
        )
    }

    #[allow(dead_code)]
    pub fn tumbnail_texture_at(
        &mut self,
        at: usize,
        ctx: &Context,
    ) -> anyhow::Result<Option<SizedTexture>> {
        match self.photo_database.get_photo_by_index(at) {
            Some(photo) => {
                let photo = photo.clone();
                if !self
                    .thumbnail_existence_cache
                    .contains(&photo.thumbnail_hash)
                {
                    return Ok(None);
                }
                let cache = self.get_cache_mut(ctx);
                Self::load_texture(
                    &photo.thumbnail_uri(),
                    ctx,
                    &mut cache.texture_cache,
                    &mut cache.pending_textures,
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
        let Some(uri) = self.texture_uri_for_photo(photo, ctx)? else {
            return Ok(None);
        };
        let cache = self.get_cache_mut(ctx);
        let result = Self::load_full_res_texture(&uri, ctx, cache);

        Self::evict_full_res_textures(ctx, cache, &uri);

        result
    }

    pub fn texture_for_blocking(
        &mut self,
        photo: &Photo,
        ctx: &Context,
    ) -> anyhow::Result<Option<SizedTexture>> {
        let uri = self.texture_uri_for_photo_blocking(photo)?;
        let cache = self.get_cache_mut(ctx);
        let result = Self::load_full_res_texture_blocking(&uri, ctx, cache);

        Self::evict_full_res_textures(ctx, cache, &uri);

        result
    }

    pub fn texture_for_photo_with_thumbail_backup(
        &mut self,
        photo: &Photo,
        ctx: &Context,
    ) -> anyhow::Result<Option<SizedTexture>> {
        let uri = self.texture_uri_for_photo(photo, ctx)?;
        let thumbnail_uri = photo.thumbnail_uri();
        let cache = self.get_cache_mut(ctx);
        let result = match uri.as_deref() {
            Some(uri) => match Self::load_full_res_texture(uri, ctx, cache) {
                Result::Ok(Some(tex)) => Ok(Some(tex)),
                _ => Ok(cache.texture_cache.get(&thumbnail_uri).copied()),
            },
            None => Ok(cache.texture_cache.get(&thumbnail_uri).copied()),
        };

        if let Some(uri) = uri.as_deref() {
            Self::evict_full_res_textures(ctx, cache, uri);
        }

        result
    }

    #[allow(dead_code)]
    pub fn texture_at(&mut self, at: usize, ctx: &Context) -> anyhow::Result<Option<SizedTexture>> {
        match self.photo_database.get_photo_by_index(at) {
            Some(photo) => {
                let photo = photo.clone();
                let Some(uri) = self.texture_uri_for_photo(&photo, ctx)? else {
                    return Ok(None);
                };
                let cache = self.get_cache_mut(ctx);
                let result = Self::load_full_res_texture(&uri, ctx, cache);

                Self::evict_full_res_textures(ctx, cache, &uri);

                result
            }
            _ => Ok(None),
        }
    }

    pub fn preload_texture(&mut self, photo: &Photo, ctx: &Context) -> anyhow::Result<()> {
        let Some(uri) = self.texture_uri_for_photo(photo, ctx)? else {
            return Ok(());
        };

        let cache = self.get_cache_mut(ctx);
        let result = Self::load_full_res_texture(&uri, ctx, cache).map(|_| ());

        Self::evict_full_res_textures(ctx, cache, &uri);

        result
    }

    pub fn preload_surrounding_textures(
        &mut self,
        current_photo: &Photo,
        ctx: &Context,
    ) -> anyhow::Result<()> {
        let current_uri = self.texture_uri_for_photo(current_photo, ctx)?;
        let preload_photos = self.surrounding_preload_photos(current_photo);
        let mut preload_uris = Vec::new();
        for photo in preload_photos {
            if let Some(uri) = self.texture_uri_for_photo(&photo, ctx)? {
                preload_uris.push(uri);
            }
        }

        let cache = self.get_cache_mut(ctx);

        let mut first_error = current_uri.as_deref().and_then(|uri| {
            Self::load_full_res_texture(uri, ctx, cache)
                .map(|_| ())
                .err()
        });

        let mut ready_polls = 0;
        let mut made_progress = false;
        for uri in preload_uris.iter() {
            if ready_polls >= FULL_RES_PRELOAD_READY_BUDGET {
                break;
            }
            if !cache.pending_textures.contains(uri) {
                continue;
            }

            ready_polls += 1;
            match Self::load_full_res_texture(uri, ctx, cache) {
                Result::Ok(Some(_)) => {
                    made_progress = true;
                }
                Result::Ok(None) => {}
                Result::Err(error) => {
                    first_error.get_or_insert(error);
                }
            }
        }

        let mut preload_attempts = 0;
        for uri in preload_uris.iter() {
            if preload_attempts >= FULL_RES_PRELOAD_START_BUDGET {
                break;
            }

            if cache.texture_cache.contains_key(uri) || cache.pending_textures.contains(uri) {
                continue;
            }

            preload_attempts += 1;
            match Self::load_full_res_texture(uri, ctx, cache) {
                Result::Ok(Some(_)) => {
                    made_progress = true;
                }
                Result::Ok(None) => {}
                Result::Err(error) => {
                    first_error.get_or_insert(error);
                }
            }
        }

        if made_progress || preload_attempts > 0 {
            ctx.request_repaint();
        }

        Self::evict_full_res_textures(ctx, cache, current_uri.as_deref().unwrap_or(""));

        if let Some(error) = first_error {
            Err(error)
        } else {
            Ok(())
        }
    }

    fn surrounding_preload_photos(&self, current_photo: &Photo) -> Vec<Photo> {
        let Some(query_result) = &self.current_query_result else {
            return Vec::new();
        };

        let mut preload_photos = Vec::new();
        let mut next_photo = current_photo.clone();
        let mut previous_photo = current_photo.clone();
        for _ in 0..FULL_RES_PRELOAD_COUNT {
            if let Some(photo) = query_result.photo_after(&next_photo) {
                preload_photos.push(photo.clone());
                next_photo = photo;
            }
            if let Some(photo) = query_result.photo_before(&previous_photo) {
                preload_photos.push(photo.clone());
                previous_photo = photo;
            }
        }

        preload_photos
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

    #[allow(dead_code)]
    fn index_for_photo(&mut self, photo: &Photo) -> Option<usize> {
        self.photo_database.get_photo_index(&photo.path)
    }

    fn texture_uri_for_photo(
        &mut self,
        photo: &Photo,
        ctx: &Context,
    ) -> anyhow::Result<Option<String>> {
        if !photo_io::is_raw_photo_path(&photo.path) {
            return Ok(Some(photo.uri()));
        }

        let rendered_path = photo_io::rendered_photo_path(&photo.path);
        if self
            .rendered_existence_cache
            .contains(&photo.thumbnail_hash)
            || rendered_path.exists()
        {
            self.rendered_existence_cache
                .insert(photo.thumbnail_hash.clone());
            return Ok(Some(photo_io::file_uri(&rendered_path)));
        }

        if self
            .pending_rendered_images
            .insert(photo.thumbnail_hash.clone())
        {
            let source_path = photo.path.clone();
            let cache_key = photo.thumbnail_hash.clone();
            let ctx = ctx.clone();
            tokio::spawn(async move {
                let result = photo_io::ensure_rendered_photo(source_path.clone()).await;
                dep_mut!(PhotoManager, |photo_manager| {
                    photo_manager.pending_rendered_images.remove(&cache_key);
                    if result.is_ok() {
                        photo_manager
                            .rendered_existence_cache
                            .insert(cache_key.clone());
                    }
                });

                if let Err(error) = result {
                    error!("Failed to render RAW photo {:?}: {:?}", source_path, error);
                }

                ctx.request_repaint();
            });
        }

        Ok(None)
    }

    fn texture_uri_for_photo_blocking(&mut self, photo: &Photo) -> anyhow::Result<String> {
        if !photo_io::is_raw_photo_path(&photo.path) {
            return Ok(photo.uri());
        }

        let rendered_path = photo_io::ensure_rendered_photo_blocking(&photo.path)?;
        self.rendered_existence_cache
            .insert(photo.thumbnail_hash.clone());
        Ok(photo_io::file_uri(&rendered_path))
    }

    fn load_full_res_texture(
        uri: &str,
        ctx: &Context,
        cache: &mut ContextCache,
    ) -> anyhow::Result<Option<SizedTexture>> {
        Self::touch_full_res_texture(cache, uri);
        let result = Self::load_texture(
            uri,
            ctx,
            &mut cache.texture_cache,
            &mut cache.pending_textures,
        );
        if result.is_err() {
            cache.full_res_accesses.remove(uri);
        }
        result
    }

    fn load_full_res_texture_blocking(
        uri: &str,
        ctx: &Context,
        cache: &mut ContextCache,
    ) -> anyhow::Result<Option<SizedTexture>> {
        Self::touch_full_res_texture(cache, uri);
        let result = Self::load_texture_blocking(
            uri,
            ctx,
            &mut cache.texture_cache,
            &mut cache.pending_textures,
        );
        if result.is_err() {
            cache.full_res_accesses.remove(uri);
        }
        result
    }

    fn touch_full_res_texture(cache: &mut ContextCache, uri: &str) {
        cache.full_res_access_counter = cache.full_res_access_counter.saturating_add(1);
        cache
            .full_res_accesses
            .insert(uri.to_string(), cache.full_res_access_counter);
    }

    fn evict_full_res_textures(ctx: &Context, cache: &mut ContextCache, protected_uri: &str) {
        let cache_capacity = FULL_RES_CACHE_CAPACITY.max(1);
        while cache.full_res_accesses.len() > cache_capacity {
            let Some(uri) = cache
                .full_res_accesses
                .iter()
                .filter(|(uri, _)| uri.as_str() != protected_uri)
                .min_by_key(|(_, last_access)| *last_access)
                .map(|(uri, _)| uri.clone())
            else {
                break;
            };

            cache.full_res_accesses.remove(&uri);
            cache.texture_cache.remove(&uri);
            cache.pending_textures.remove(&uri);
            ctx.forget_image(&uri);
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
                Ok(Some(*texture))
            }
            None => {
                let texture = ctx.try_load_texture(
                    uri,
                    eframe::egui::TextureOptions::default(),
                    eframe::egui::SizeHint::Scale(OrderedFloat(1.0)),
                );

                match texture {
                    Result::Ok(eframe::egui::load::TexturePoll::Pending { size: _ }) => {
                        pending_textures.insert(uri.to_string());
                        Ok(None)
                    }
                    Result::Ok(eframe::egui::load::TexturePoll::Ready { texture }) => {
                        pending_textures.remove(uri);
                        texture_cache.insert(uri.to_string(), texture);
                        Ok(Some(texture))
                    }
                    Result::Err(err) => {
                        pending_textures.remove(uri);
                        error!("Failed to load texture {:?}", err);
                        Err(anyhow!(err))
                    }
                }
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
                        pending_textures.remove(uri);
                        texture_cache.insert(uri.to_string(), texture);
                        Ok(Some(texture))
                    }
                    Result::Err(err) => {
                        pending_textures.remove(uri);
                        Err(anyhow!(err))
                    }
                }
            }
        }
    }

    fn add_discovered_photo_paths(photo_paths: &[PathBuf]) {
        dep_mut!(PhotoManager, |photo_manager| {
            for photo_path in photo_paths {
                if photo_manager.photo_exists(photo_path) {
                    continue;
                }

                photo_manager
                    .photo_database
                    .add_photo(Photo::discovered(photo_path.clone()));
            }

            photo_manager.sort_and_regroup();
        });
    }

    async fn resolve_photo_metadata(photo_paths: Vec<PathBuf>) {
        for photo_path in photo_paths {
            match Photo::new_async(photo_path.clone()).await {
                Result::Ok(photo) => dep_mut!(PhotoManager, |photo_manager| {
                    if photo_manager.photo_exists(&photo.path) {
                        photo_manager.photo_database.update_photo(photo);
                    }
                }),
                Err(err) => {
                    error!(
                        "Failed to load photo metadata: {:?} - {:?}",
                        photo_path, err
                    );
                }
            }
        }

        dep_mut!(PhotoManager, |photo_manager| {
            photo_manager.sort_and_regroup();
        });
    }

    async fn gen_thumbnails(photo_paths: Vec<PathBuf>) -> anyhow::Result<()> {
        let (standard_photo_paths, raw_photo_paths): (Vec<PathBuf>, Vec<PathBuf>) = photo_paths
            .into_iter()
            .partition(|path| !photo_io::is_raw_photo_path(path));
        let thumbnail_dir = Dirs::Thumbnails.path();

        Self::gen_thumbnail_batch(standard_photo_paths, thumbnail_dir.clone()).await?;
        Self::gen_thumbnail_batch(raw_photo_paths, thumbnail_dir).await
    }

    async fn gen_thumbnail_batch(
        photo_paths: Vec<PathBuf>,
        thumbnail_dir: PathBuf,
    ) -> anyhow::Result<()> {
        let partitions = utils::partition_iterator(photo_paths.into_iter(), 16);
        let mut tasks = Vec::new();

        for partition in partitions {
            if partition.is_empty() {
                continue;
            }

            let thumbnail_dir: PathBuf = thumbnail_dir.clone();
            tasks.push(tokio::task::spawn(async move {
                for photo in partition {
                    let res: Result<(), anyhow::Error> =
                        Self::gen_thumbnail(&photo, &thumbnail_dir).await;
                    if res.is_err() {
                        // TODO: Handle this better
                        error!("{:?}", res);
                        // panic!("{:?}", res);
                    }
                }
            }));
        }

        for task in tasks {
            if let Err(err) = task.await {
                error!("Thumbnail generation task failed: {:?}", err);
            }
        }

        Ok(())
    }

    async fn gen_thumbnail(photo_path: &PathBuf, thumbnail_dir: &PathBuf) -> anyhow::Result<()> {
        let file_name = photo_path.file_name();
        let extension = photo_io::thumbnail_extension(photo_path);

        if file_name.is_some() {
            if photo_io::is_supported_photo_path(photo_path) {
                // TODO: incorporate the last modified date of the photo into the hash
                let hash = hash64(&photo_path.to_string_lossy()).to_string();

                let mut thumbnail_path = thumbnail_dir.join(&hash);
                thumbnail_path.set_extension(&extension);

                if thumbnail_path.exists() {
                    // info!("Thumbnail already exists for: {:?}", &photo_path);
                    dep_mut!(PhotoManager, |photo_manager| {
                        photo_manager.thumbnail_existence_cache.insert(hash);
                    });

                    return Ok(());
                } else {
                    info!("Generating thumbnail: {:?}", &thumbnail_path);
                }

                let img = if photo_io::is_raw_photo_path(photo_path) {
                    let photo_path = photo_path.clone();
                    spawn_blocking(move || photo_io::decode_raw_thumbnail_image(&photo_path))
                        .await??
                } else {
                    let file_bytes = tokio::fs::read(photo_path).await?;
                    spawn_blocking(move || {
                        image::ImageReader::new(std::io::Cursor::new(file_bytes))
                            .with_guessed_format()?
                            .decode()
                    })
                    .await??
                };

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

                match extension.as_str() {
                    "jpg" | "jpeg" => match dst_image.pixel_type() {
                        fr::PixelType::U8x4 => {
                            let buffer = dst_image.buffer();
                            let rgb_data: Vec<u8> = buffer
                                .chunks_exact(4)
                                .flat_map(|chunk| chunk[0..3].iter().copied())
                                .collect();
                            JpegEncoder::new_with_quality(&mut result_buf, 60).write_image(
                                &rgb_data,
                                dst_width,
                                dst_height,
                                ExtendedColorType::Rgb8,
                            )?;
                        }
                        _ => {
                            JpegEncoder::new_with_quality(&mut result_buf, 60).write_image(
                                dst_image.buffer(),
                                dst_width,
                                dst_height,
                                ExtendedColorType::Rgb8,
                            )?;
                        }
                    },
                    "png" => match dst_image.pixel_type() {
                        fr::PixelType::U8x4 => {
                            PngEncoder::new(&mut result_buf).write_image(
                                dst_image.buffer(),
                                dst_width,
                                dst_height,
                                ExtendedColorType::Rgba8,
                            )?;
                        }
                        _ => {
                            PngEncoder::new(&mut result_buf).write_image(
                                dst_image.buffer(),
                                dst_width,
                                dst_height,
                                ExtendedColorType::Rgb8,
                            )?;
                        }
                    },
                    _ => {
                        return Err(anyhow::anyhow!("Invalid file extension"));
                    }
                }

                let buf = result_buf.into_inner()?;

                let mut file = TokioFile::create(&thumbnail_path).await?;
                file.write_all(&buf).await?;
                file.sync_all().await?;

                info!("Thumbnail generated: {:?}", &thumbnail_path);

                dep_mut!(PhotoManager, |photo_manager| {
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

    #[allow(dead_code)]
    pub fn clear_current_filter(&mut self) {
        self.current_filter = PhotoQuery::default();
    }

    pub fn albums_iter(&self) -> impl Iterator<Item = &Album> {
        self.photo_database.albums_iter()
    }

    pub fn album_photos_iter(&mut self, album_id: &AlbumId) -> impl Iterator<Item = &PathBuf> {
        self.photo_database.album_photos_iter(album_id)
    }

    pub fn add_to_album(&mut self, album_id: &AlbumId, photo_path: &PathBuf) {
        self.photo_database.add_to_album(album_id, photo_path);
    }

    pub fn remove_from_album(&mut self, album_id: &AlbumId, photo_path: &PathBuf) {
        self.photo_database.remove_from_album(album_id, photo_path);
    }

    pub fn get_photo_albums(&self, photo_path: &PathBuf) -> HashSet<AlbumId> {
        self.photo_database.get_photo_albums(photo_path)
    }

    pub fn create_album(&mut self, album_name: &String) -> Option<AlbumId> {
        self.photo_database.create_album(album_name)
    }

    pub fn insert_album(&mut self, album: Album) -> Option<AlbumId> {
        self.photo_database.insert_album(album)
    }

    pub fn rename_album(&mut self, album_id: &AlbumId, new_name: &String) {
        self.photo_database.rename_album(album_id, new_name);
    }

    pub fn delete_album(&mut self, album_id: &AlbumId) {
        self.photo_database.delete_album(album_id);
    }
}

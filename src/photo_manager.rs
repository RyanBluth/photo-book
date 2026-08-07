use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    io::BufWriter,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use glob::MatchOptions;

use chrono::{DateTime, Utc};
use eframe::egui::{ColorImage, Context, TextureHandle, TextureOptions, load::SizedTexture};
use egui::emath::OrderedFloat;
use image::{
    ExtendedColorType,
    codecs::{jpeg::JpegEncoder, png::PngEncoder},
};
use log::{error, info};
use tokio::sync::oneshot;
use tokio::sync::oneshot::error::TryRecvError;
use tokio::task::spawn_blocking;
use tokio::{fs::File as TokioFile, io::AsyncWriteExt};

use crate::{
    app_status::{AppJob, AppJobStatus, AppStatus},
    cancellation::CancellationToken,
    dep, dep_mut,
    dirs::Dirs,
    image_utils::{decode_oriented_image, path_version_key},
    model::{
        album::{Album, AlbumId},
        photo_adjustments::PhotoAdjustments,
        photo_grouping::PhotoGrouping,
    },
    photo::{Photo, PhotoRating},
    photo_database::{PhotoDatabase, PhotoQuery, PhotoQueryResult, PhotoSortCriteria},
};

use anyhow::anyhow;
use fr::CpuExtensions;
use image::ImageEncoder;

use fast_image_resize::{self as fr, ResizeOptions};
use fxhash::hash64;

use crate::utils;

const THUMBNAIL_SIZE: f32 = 512.0;
const FULL_RES_PRELOAD_COUNT: usize = 4;
const FULL_RES_PRELOAD_START_BUDGET: usize = 4;
const FULL_RES_PRELOAD_READY_BUDGET: usize = 2;
const FULL_RES_CACHE_CAPACITY: usize = 20;
const ADJUSTED_THUMBNAIL_CACHE_CAPACITY: usize = 512;
const EXPORT_TEXTURE_LOAD_TIMEOUT: Duration = Duration::from_secs(60);
const EXPORT_TEXTURE_POLL_INTERVAL: Duration = Duration::from_millis(1);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PhotoTextureResolution {
    Full,
    Thumbnail,
}

#[derive(Clone, Copy, Debug)]
enum PhotoTextureAdjustments<'a> {
    Stored,
    Explicit(&'a PhotoAdjustments),
    None,
}

/// Controls how [`PhotoManager::texture_for`] loads a photo texture.
#[derive(Clone, Copy, Debug)]
pub struct PhotoTextureOptions<'a> {
    resolution: PhotoTextureResolution,
    adjustments: PhotoTextureAdjustments<'a>,
    thumbnail_fallback: bool,
}

impl Default for PhotoTextureOptions<'_> {
    fn default() -> Self {
        Self::full_resolution()
    }
}

impl<'a> PhotoTextureOptions<'a> {
    /// Loads the full-resolution image using the photo's stored adjustments.
    pub fn full_resolution() -> Self {
        Self {
            resolution: PhotoTextureResolution::Full,
            adjustments: PhotoTextureAdjustments::Stored,
            thumbnail_fallback: false,
        }
    }

    /// Loads the thumbnail using the photo's stored adjustments.
    pub fn thumbnail() -> Self {
        Self {
            resolution: PhotoTextureResolution::Thumbnail,
            ..Self::full_resolution()
        }
    }

    /// Uses these adjustments instead of the photo's stored adjustments.
    pub fn with_adjustments(mut self, adjustments: &'a PhotoAdjustments) -> Self {
        self.adjustments = PhotoTextureAdjustments::Explicit(adjustments);
        self
    }

    /// Loads the source image without applying adjustments.
    pub fn without_adjustments(mut self) -> Self {
        self.adjustments = PhotoTextureAdjustments::None;
        self
    }

    /// Returns a thumbnail while the full-resolution texture is still loading.
    pub fn with_thumbnail_fallback(mut self) -> Self {
        self.thumbnail_fallback = true;
        self
    }
}

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

#[derive(Default)]
struct ContextCache {
    texture_cache: HashMap<String, SizedTexture>,
    adjusted_texture_cache: HashMap<String, TextureHandle>,
    pending_textures: HashSet<String>,
    pending_adjusted_textures:
        HashMap<String, oneshot::Receiver<std::result::Result<AdjustedTextureImage, String>>>,
    full_res_accesses: HashMap<String, u64>,
    full_res_access_counter: u64,
    adjusted_thumbnail_accesses: HashMap<String, u64>,
    adjusted_thumbnail_access_counter: u64,
}

#[derive(Debug)]
pub struct PhotoManager {
    current_grouping: PhotoGrouping,
    current_filter: PhotoQuery,
    caches: Vec<(Context, ContextCache)>,
    thumbnail_existence_cache: HashSet<String>,
    current_query_result: Option<Arc<PhotoQueryResult>>,
    pub photo_database: PhotoDatabase,
}

impl PhotoManager {
    pub fn new() -> Self {
        Self {
            current_grouping: PhotoGrouping::default(),
            current_filter: PhotoQuery::default(),
            caches: Vec::new(),
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
        self.caches.clear();
        self.thumbnail_existence_cache.clear();
        self.current_query_result = None;
        self.photo_database.clear();
    }

    pub fn remove_cached_textures_for_context(&mut self, ctx: &Context) {
        self.caches.retain(|(context, _)| context != ctx);
    }

    /// Moves the complete texture cache for `ctx` into `destination`.
    ///
    /// This is useful when a local export manager prepares textures without holding the global
    /// manager lock, then needs the resulting handles to be visible to normal render paths.
    /// Any existing destination cache for the same context is replaced.
    pub fn transfer_cached_textures_for_context_to(
        &mut self,
        ctx: &Context,
        destination: &mut Self,
    ) -> bool {
        let Some(index) = self.caches.iter().position(|(context, _)| context == ctx) else {
            return false;
        };

        let (context, cache) = self.caches.swap_remove(index);
        destination.remove_cached_textures_for_context(ctx);
        destination.caches.push((context, cache));
        true
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

    pub fn load_directory(path: PathBuf) -> anyhow::Result<()> {
        dep_mut!(AppStatus, |app_status| {
            app_status.update(AppJob::DiscoveringPhotos, AppJobStatus::Indefinite);
        });
        tokio::spawn(async move {
            let glob_patterns = [
                format!("{}/**/*.jpg", path.to_string_lossy()),
                format!("{}/**/*.jpeg", path.to_string_lossy()),
                format!("{}/**/*.png", path.to_string_lossy()),
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
                    if (lowercase_extension == "jpg"
                        || lowercase_extension == "jpeg"
                        || lowercase_extension == "png")
                        && !dep!(PhotoManager, |pm| pm.photo_exists(path))
                    {
                        Some(path.clone())
                    } else {
                        None
                    }
                })
                .collect();

            for photo_path in pending_photos {
                match Photo::new_async(photo_path.clone()).await {
                    Ok(photo) => {
                        dep_mut!(PhotoManager, |photo_manager| {
                            photo_manager.photo_database.add_photo(photo);
                        });
                    }
                    Err(err) => {
                        error!("Failed to load photo: {:?} - {:?}", photo_path, err);
                    }
                }
            }

            dep_mut!(PhotoManager, |photo_manager| {
                photo_manager
                    .photo_database
                    .sort_photos(PhotoSortCriteria::Date);
            });

            let photo_paths: Vec<PathBuf> = dep!(PhotoManager, |photo_manager| photo_manager
                .photo_database
                .get_all_photo_paths());

            dep_mut!(AppStatus, |app_status| {
                app_status.complete(AppJob::DiscoveringPhotos);
            });

            let _ = Self::gen_thumbnails(photo_paths);
        });

        Ok(())
    }

    pub fn load_photos(&self, photos: Vec<Photo>) {
        tokio::spawn(async move {
            let mut photos_since_regroup: usize = 0;
            let filtered_photos: Vec<Photo> = photos
                .into_iter()
                .filter(|photo| !dep!(PhotoManager, |pm| pm.photo_exists(&photo.path)))
                .collect();

            let num_photos = filtered_photos.len();
            dep_mut!(PhotoManager, |photo_manager| {
                for photo in filtered_photos {
                    photo_manager.photo_database.add_photo(photo);

                    photos_since_regroup += 1;

                    if photos_since_regroup > 500 || num_photos == photos_since_regroup {
                        photos_since_regroup = 0;
                        photo_manager.sort_and_regroup();
                    }
                }
            });

            let (photo_paths, _) = dep_mut!(PhotoManager, |photo_manager| {
                let photo_paths: Vec<PathBuf> = photo_manager.photo_database.get_all_photo_paths();
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

    pub fn grouped_photos(&mut self) -> Arc<PhotoQueryResult> {
        let mut query = self.current_filter.clone();

        // Ensure grouping is set
        query.grouping = self.current_grouping;
        let query_result = self.photo_database.query_photos(&query);

        if let Some(current_query_result) = &self.current_query_result
            && query_result.id() == current_query_result.id()
        {
            Arc::clone(current_query_result)
        } else {
            self.current_query_result = Some(Arc::clone(&query_result));
            query_result
        }
    }

    pub fn photo_grouping(&self) -> PhotoGrouping {
        self.current_grouping
    }

    /// Change the photo grouping to given one
    pub fn group_photos_by(&mut self, photos_grouping: PhotoGrouping) -> Arc<PhotoQueryResult> {
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

    fn thumbnail_exists(&mut self, photo: &Photo) -> bool {
        if self
            .thumbnail_existence_cache
            .contains(&photo.thumbnail_hash)
        {
            return true;
        }

        let Ok(thumbnail_path) = photo.thumbnail_path() else {
            return false;
        };

        if thumbnail_path.exists() {
            self.thumbnail_existence_cache
                .insert(photo.thumbnail_hash.clone());
            true
        } else {
            false
        }
    }

    fn mark_thumbnail_available(&mut self, thumbnail_hash: String, thumbnail_path: &Path) {
        self.thumbnail_existence_cache.insert(thumbnail_hash);
        self.invalidate_thumbnail_textures(thumbnail_path);
    }

    fn invalidate_thumbnail_textures(&mut self, thumbnail_path: &Path) {
        let thumbnail_uri = format!("file://{}", thumbnail_path.display());
        let adjusted_prefix = format!("adjusted_thumbnail:{}:", thumbnail_uri);

        for (ctx, cache) in &mut self.caches {
            cache.texture_cache.remove(&thumbnail_uri);
            cache.pending_textures.remove(&thumbnail_uri);
            ctx.forget_image(&thumbnail_uri);

            cache
                .adjusted_texture_cache
                .retain(|cache_key, _| !cache_key.starts_with(&adjusted_prefix));
            cache
                .adjusted_thumbnail_accesses
                .retain(|cache_key, _| !cache_key.starts_with(&adjusted_prefix));
            cache
                .pending_adjusted_textures
                .retain(|cache_key, _| !cache_key.starts_with(&adjusted_prefix));
        }
    }

    /// Loads a texture according to `options`.
    ///
    /// Returns `None` while asynchronous work is pending or a requested thumbnail is unavailable.
    pub fn texture_for(
        &mut self,
        photo: &Photo,
        ctx: &Context,
        options: PhotoTextureOptions<'_>,
    ) -> anyhow::Result<Option<SizedTexture>> {
        let adjustments = match options.adjustments {
            PhotoTextureAdjustments::Stored => Cow::Owned(self.get_photo_adjustments(&photo.path)),
            PhotoTextureAdjustments::Explicit(adjustments) => Cow::Borrowed(adjustments),
            PhotoTextureAdjustments::None => Cow::Owned(PhotoAdjustments::default()),
        };
        let adjustments = adjustments.as_ref();
        let allow_adjustment_jobs = !ctx.input(|input| input.pointer.primary_down());

        match options.resolution {
            PhotoTextureResolution::Thumbnail => {
                if !self.thumbnail_exists(photo) {
                    return Ok(None);
                }

                let cache = self.get_cache_mut(ctx);
                if adjustments.is_identity() {
                    Self::load_unadjusted_texture(&photo.thumbnail_uri(), ctx, cache)
                } else {
                    Self::load_adjusted_thumbnail_texture(
                        photo,
                        adjustments,
                        allow_adjustment_jobs,
                        ctx,
                        cache,
                    )
                }
            }
            PhotoTextureResolution::Full => {
                let uri = photo.uri();
                let thumbnail_exists = options.thumbnail_fallback && self.thumbnail_exists(photo);
                let max_texture_side = ctx.input(|input| input.max_texture_side as u32);
                let protected_uri = if adjustments.is_identity() {
                    uri.clone()
                } else {
                    adjusted_texture_cache_key(&uri, &path_version_key(&photo.path), adjustments)
                };
                let cache = self.get_cache_mut(ctx);
                let full_res_result = if adjustments.is_identity() {
                    Self::load_full_res_texture(&uri, ctx, cache)
                } else {
                    Self::load_adjusted_full_res_texture(
                        photo,
                        adjustments,
                        max_texture_side,
                        allow_adjustment_jobs,
                        ctx,
                        cache,
                    )
                };

                let result = if options.thumbnail_fallback {
                    match full_res_result {
                        Ok(Some(texture)) => Ok(Some(texture)),
                        _ if !thumbnail_exists => Ok(None),
                        _ if adjustments.is_identity() || !allow_adjustment_jobs => {
                            Self::load_unadjusted_texture(&photo.thumbnail_uri(), ctx, cache)
                        }
                        _ => Self::load_adjusted_thumbnail_texture(
                            photo,
                            adjustments,
                            true,
                            ctx,
                            cache,
                        ),
                    }
                } else {
                    full_res_result
                };

                Self::evict_full_res_textures(ctx, cache, &protected_uri);
                result
            }
        }
    }

    /// Loads a full-resolution texture for an off-thread export and returns only when it is ready.
    pub fn texture_for_export(
        &mut self,
        photo: &Photo,
        adjustments: &PhotoAdjustments,
        ctx: &Context,
    ) -> anyhow::Result<SizedTexture> {
        self.texture_for_export_cancellable(photo, adjustments, ctx, None)
    }

    pub fn texture_for_export_cancellable(
        &mut self,
        photo: &Photo,
        adjustments: &PhotoAdjustments,
        ctx: &Context,
        cancellation: Option<&CancellationToken>,
    ) -> anyhow::Result<SizedTexture> {
        let uri = photo.uri();
        let max_texture_side = ctx.input(|input| input.max_texture_side as u32);
        let protected_uri = if adjustments.is_identity() {
            uri.clone()
        } else {
            adjusted_texture_cache_key(&uri, &path_version_key(&photo.path), adjustments)
        };
        let cache = self.get_cache_mut(ctx);
        let result = if adjustments.is_identity() {
            Self::load_full_res_texture_for_export(&uri, ctx, cache, cancellation)
        } else {
            Self::load_adjusted_full_res_texture_for_export(
                photo,
                adjustments,
                max_texture_side,
                ctx,
                cache,
                cancellation,
            )
        };

        Self::evict_full_res_textures(ctx, cache, &protected_uri);
        result
    }

    pub fn preload_texture(&mut self, photo: &Photo, ctx: &Context) -> anyhow::Result<()> {
        let uri = photo.uri();

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
        let current_uri = current_photo.uri();
        let preload_uris = self.surrounding_preload_uris(current_photo);

        let cache = self.get_cache_mut(ctx);

        let mut first_error = Self::load_full_res_texture(&current_uri, ctx, cache)
            .map(|_| ())
            .err();

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
                Ok(Some(_)) => {
                    made_progress = true;
                }
                Ok(None) => {}
                Err(error) => {
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
                Ok(Some(_)) => {
                    made_progress = true;
                }
                Ok(None) => {}
                Err(error) => {
                    first_error.get_or_insert(error);
                }
            }
        }

        if made_progress || preload_attempts > 0 {
            ctx.request_repaint();
        }

        Self::evict_full_res_textures(ctx, cache, &current_uri);

        if let Some(error) = first_error {
            Err(error)
        } else {
            Ok(())
        }
    }

    fn surrounding_preload_uris(&self, current_photo: &Photo) -> Vec<String> {
        let Some(query_result) = &self.current_query_result else {
            return Vec::new();
        };

        let mut preload_uris = Vec::new();
        let mut next_photo = current_photo.clone();
        let mut previous_photo = current_photo.clone();
        for _ in 0..FULL_RES_PRELOAD_COUNT {
            if let Some(photo) = query_result.photo_after(&next_photo) {
                preload_uris.push(photo.uri());
                next_photo = photo;
            }
            if let Some(photo) = query_result.photo_before(&previous_photo) {
                preload_uris.push(photo.uri());
                previous_photo = photo;
            }
        }

        preload_uris
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

    fn load_full_res_texture_for_export(
        uri: &str,
        ctx: &Context,
        cache: &mut ContextCache,
        cancellation: Option<&CancellationToken>,
    ) -> anyhow::Result<SizedTexture> {
        Self::touch_full_res_texture(cache, uri);
        let result = Self::load_texture_for_export(
            uri,
            ctx,
            &mut cache.texture_cache,
            &mut cache.pending_textures,
            cancellation,
        );
        if result.is_err() {
            cache.full_res_accesses.remove(uri);
        }
        result
    }

    fn load_adjusted_full_res_texture(
        photo: &Photo,
        adjustments: &PhotoAdjustments,
        max_texture_side: u32,
        allow_start: bool,
        ctx: &Context,
        cache: &mut ContextCache,
    ) -> anyhow::Result<Option<SizedTexture>> {
        let uri = photo.uri();
        let source_version = path_version_key(&photo.path);
        let cache_key = adjusted_texture_cache_key(&uri, &source_version, adjustments);
        if let Some(texture) = cache.adjusted_texture_cache.get(&cache_key).cloned() {
            Self::touch_full_res_texture(cache, &cache_key);
            return Ok(Some(SizedTexture::from_handle(&texture)));
        }

        Self::touch_full_res_texture(cache, &cache_key);

        let result = Self::load_adjusted_texture(
            photo.path.clone(),
            cache_key.clone(),
            adjustments,
            max_texture_side,
            allow_start,
            ctx,
            cache,
        );

        if result.is_err() {
            cache.full_res_accesses.remove(&cache_key);
        }

        result
    }

    fn load_adjusted_full_res_texture_for_export(
        photo: &Photo,
        adjustments: &PhotoAdjustments,
        max_texture_side: u32,
        ctx: &Context,
        cache: &mut ContextCache,
        cancellation: Option<&CancellationToken>,
    ) -> anyhow::Result<SizedTexture> {
        let uri = photo.uri();
        let source_version = path_version_key(&photo.path);
        let cache_key = adjusted_texture_cache_key(&uri, &source_version, adjustments);
        Self::touch_full_res_texture(cache, &cache_key);

        let result = Self::load_adjusted_texture_for_export(
            photo.path.clone(),
            cache_key.clone(),
            adjustments,
            max_texture_side,
            ctx,
            cache,
            cancellation,
        );

        if result.is_err() {
            cache.full_res_accesses.remove(&cache_key);
        }

        result
    }

    fn load_adjusted_thumbnail_texture(
        photo: &Photo,
        adjustments: &PhotoAdjustments,
        allow_start: bool,
        ctx: &Context,
        cache: &mut ContextCache,
    ) -> anyhow::Result<Option<SizedTexture>> {
        let thumbnail_uri = photo.thumbnail_uri();
        let thumbnail_path = photo.thumbnail_path()?;
        let source_version = path_version_key(&thumbnail_path);
        let cache_key = adjusted_thumbnail_cache_key(&thumbnail_uri, &source_version, adjustments);
        Self::touch_adjusted_thumbnail_texture(cache, &cache_key);

        let result = Self::load_adjusted_texture(
            thumbnail_path,
            cache_key.clone(),
            adjustments,
            0,
            allow_start,
            ctx,
            cache,
        );

        if matches!(result, Ok(Some(_))) {
            Self::evict_adjusted_thumbnail_textures(ctx, cache, &cache_key);
        }

        if result.is_err() {
            cache.adjusted_thumbnail_accesses.remove(&cache_key);
        }

        result
    }

    fn load_adjusted_texture(
        source_path: PathBuf,
        cache_key: String,
        adjustments: &PhotoAdjustments,
        max_texture_side: u32,
        allow_start: bool,
        ctx: &Context,
        cache: &mut ContextCache,
    ) -> anyhow::Result<Option<SizedTexture>> {
        if let Some(texture) = cache.adjusted_texture_cache.get(&cache_key) {
            return Ok(Some(SizedTexture::from_handle(texture)));
        }

        if let Some(result) = Self::poll_adjusted_texture(cache, &cache_key) {
            let image = result.map_err(anyhow::Error::msg)?;
            let color_image = ColorImage::from_rgba_unmultiplied(image.size, &image.rgba);
            let texture =
                ctx.load_texture(cache_key.clone(), color_image, TextureOptions::default());
            let sized_texture = SizedTexture::from_handle(&texture);
            cache.adjusted_texture_cache.insert(cache_key, texture);
            return Ok(Some(sized_texture));
        }

        if cache.pending_adjusted_textures.contains_key(&cache_key) {
            return Ok(None);
        }

        if !allow_start {
            return Ok(None);
        }

        let (sender, receiver) = oneshot::channel();
        Self::spawn_adjusted_texture_job(
            source_path,
            adjustments.clone(),
            max_texture_side,
            sender,
            ctx.clone(),
        );
        cache.pending_adjusted_textures.insert(cache_key, receiver);

        Ok(None)
    }

    fn load_adjusted_texture_for_export(
        source_path: PathBuf,
        cache_key: String,
        adjustments: &PhotoAdjustments,
        max_texture_side: u32,
        ctx: &Context,
        cache: &mut ContextCache,
        cancellation: Option<&CancellationToken>,
    ) -> anyhow::Result<SizedTexture> {
        if !cache.adjusted_texture_cache.contains_key(&cache_key) {
            // Replace any asynchronous receiver: its worker may be queued on the Tokio pool
            // currently occupied by the export caller.
            let (sender, receiver) = oneshot::channel();
            Self::spawn_adjusted_texture_job_on_thread(
                source_path.clone(),
                adjustments.clone(),
                max_texture_side,
                sender,
                None,
            );
            cache
                .pending_adjusted_textures
                .insert(cache_key.clone(), receiver);
        }

        let result = wait_for_texture(
            &cache_key,
            EXPORT_TEXTURE_LOAD_TIMEOUT,
            EXPORT_TEXTURE_POLL_INTERVAL,
            cancellation,
            || {
                Self::load_adjusted_texture(
                    source_path.clone(),
                    cache_key.clone(),
                    adjustments,
                    max_texture_side,
                    false,
                    ctx,
                    cache,
                )
            },
        );

        if result.is_err() {
            cache.pending_adjusted_textures.remove(&cache_key);
        }

        result
    }

    fn poll_adjusted_texture(
        cache: &mut ContextCache,
        cache_key: &str,
    ) -> Option<std::result::Result<AdjustedTextureImage, String>> {
        let result = match cache.pending_adjusted_textures.get_mut(cache_key) {
            Some(receiver) => match receiver.try_recv() {
                Ok(result) => Some(result),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Closed) => {
                    Some(Err("adjusted texture worker stopped".to_owned()))
                }
            },
            None => None,
        };

        if result.is_some() {
            cache.pending_adjusted_textures.remove(cache_key);
        }

        result
    }

    fn spawn_adjusted_texture_job(
        source_path: PathBuf,
        adjustments: PhotoAdjustments,
        max_texture_side: u32,
        sender: oneshot::Sender<std::result::Result<AdjustedTextureImage, String>>,
        repaint_ctx: Context,
    ) {
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            let load = move || adjusted_texture_image(source_path, adjustments, max_texture_side);
            handle.spawn(async move {
                let result = spawn_blocking(load)
                    .await
                    .map_err(|error| error.to_string())
                    .and_then(|result| result);
                let _ = sender.send(result);
                repaint_ctx.request_repaint();
            });
        } else {
            Self::spawn_adjusted_texture_job_on_thread(
                source_path,
                adjustments,
                max_texture_side,
                sender,
                Some(repaint_ctx),
            );
        }
    }

    fn spawn_adjusted_texture_job_on_thread(
        source_path: PathBuf,
        adjustments: PhotoAdjustments,
        max_texture_side: u32,
        sender: oneshot::Sender<std::result::Result<AdjustedTextureImage, String>>,
        repaint_ctx: Option<Context>,
    ) {
        std::thread::spawn(move || {
            let result = adjusted_texture_image(source_path, adjustments, max_texture_side);
            let _ = sender.send(result);
            if let Some(ctx) = repaint_ctx {
                ctx.request_repaint();
            }
        });
    }

    fn touch_full_res_texture(cache: &mut ContextCache, uri: &str) {
        cache.full_res_access_counter = cache.full_res_access_counter.saturating_add(1);
        cache
            .full_res_accesses
            .insert(uri.to_string(), cache.full_res_access_counter);
    }

    fn touch_adjusted_thumbnail_texture(cache: &mut ContextCache, uri: &str) {
        cache.adjusted_thumbnail_access_counter =
            cache.adjusted_thumbnail_access_counter.saturating_add(1);
        cache
            .adjusted_thumbnail_accesses
            .insert(uri.to_string(), cache.adjusted_thumbnail_access_counter);
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
            cache.adjusted_texture_cache.remove(&uri);
            cache.pending_textures.remove(&uri);
            cache.pending_adjusted_textures.remove(&uri);
            ctx.forget_image(&uri);
        }
    }

    fn evict_adjusted_thumbnail_textures(
        ctx: &Context,
        cache: &mut ContextCache,
        protected_uri: &str,
    ) {
        let cache_capacity = ADJUSTED_THUMBNAIL_CACHE_CAPACITY.max(1);
        while cache.adjusted_thumbnail_accesses.len() > cache_capacity {
            let Some(uri) = cache
                .adjusted_thumbnail_accesses
                .iter()
                .filter(|(uri, _)| uri.as_str() != protected_uri)
                .min_by_key(|(_, last_access)| *last_access)
                .map(|(uri, _)| uri.clone())
            else {
                break;
            };

            cache.adjusted_thumbnail_accesses.remove(&uri);
            cache.adjusted_texture_cache.remove(&uri);
            cache.pending_adjusted_textures.remove(&uri);
            ctx.forget_image(&uri);
        }
    }

    fn load_unadjusted_texture(
        uri: &str,
        ctx: &Context,
        cache: &mut ContextCache,
    ) -> anyhow::Result<Option<SizedTexture>> {
        Self::load_texture(
            uri,
            ctx,
            &mut cache.texture_cache,
            &mut cache.pending_textures,
        )
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
                    Ok(eframe::egui::load::TexturePoll::Pending { size: _ }) => {
                        pending_textures.insert(uri.to_string());
                        Ok(None)
                    }
                    Ok(eframe::egui::load::TexturePoll::Ready { texture }) => {
                        pending_textures.remove(uri);
                        texture_cache.insert(uri.to_string(), texture);
                        Ok(Some(texture))
                    }
                    Err(err) => {
                        pending_textures.remove(uri);
                        error!("Failed to load texture {:?}", err);
                        Err(anyhow!(err))
                    }
                }
            }
        }
    }

    fn load_texture_for_export(
        uri: &str,
        ctx: &Context,
        texture_cache: &mut HashMap<String, SizedTexture>,
        pending_textures: &mut HashSet<String>,
        cancellation: Option<&CancellationToken>,
    ) -> anyhow::Result<SizedTexture> {
        let result = wait_for_texture(
            uri,
            EXPORT_TEXTURE_LOAD_TIMEOUT,
            EXPORT_TEXTURE_POLL_INTERVAL,
            cancellation,
            || Self::load_texture(uri, ctx, texture_cache, pending_textures),
        );

        if result.is_err() {
            pending_textures.remove(uri);
        }

        result
    }

    fn gen_thumbnails(photo_paths: Vec<PathBuf>) -> anyhow::Result<()> {
        if !photo_paths.is_empty() {
            dep_mut!(AppStatus, |app_status| {
                app_status.start_finite(AppJob::GeneratingThumbnails, photo_paths.len());
            });
        }

        let thumbnail_dir = Dirs::Thumbnails.path();

        let partitions = utils::partition_iterator(photo_paths.into_iter(), 16);

        for partition in partitions {
            let thumbnail_dir: PathBuf = thumbnail_dir.clone();
            tokio::task::spawn(async move {
                for photo in partition {
                    let res: Result<(), anyhow::Error> =
                        Self::gen_thumbnail(&photo, &thumbnail_dir).await;
                    dep_mut!(AppStatus, |app_status| {
                        app_status.increment_and_complete_if_done(AppJob::GeneratingThumbnails);
                    });
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

    async fn gen_thumbnail(photo_path: &Path, thumbnail_dir: &Path) -> anyhow::Result<()> {
        let file_name = photo_path.file_name();
        let extension = photo_path.extension();

        if let (Some(_), Some(extension)) = (file_name, extension)
            && (extension.eq_ignore_ascii_case("jpg")
                || extension.eq_ignore_ascii_case("png")
                || extension.eq_ignore_ascii_case("jpeg"))
        {
            // TODO: incorporate the last modified date of the photo into the hash
            let hash = Photo::thumbnail_hash_for_path(photo_path);

            let mut thumbnail_path = thumbnail_dir.join(&hash);
            thumbnail_path.set_extension(extension);

            if thumbnail_path.exists() {
                // info!("Thumbnail already exists for: {:?}", &photo_path);
                dep_mut!(PhotoManager, |photo_manager| {
                    photo_manager.mark_thumbnail_available(hash, &thumbnail_path);
                });

                return Ok(());
            } else {
                info!("Generating thumbnail: {:?}", &thumbnail_path);
            }

            let file_bytes = tokio::fs::read(photo_path).await?;
            let img = spawn_blocking(move || {
                let format = image::guess_format(&file_bytes)?;
                let reader =
                    image::ImageReader::with_format(std::io::Cursor::new(file_bytes), format);
                let mut decoder = reader.into_decoder()?;
                let orientation = image::ImageDecoder::orientation(&mut decoder)?;
                let mut image = image::DynamicImage::from_decoder(decoder)?;
                image.apply_orientation(orientation);
                std::result::Result::<_, image::ImageError>::Ok(image)
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

                Ok::<_, anyhow::Error>(dst_image)
            })
            .await??;

            // Write destination image as PNG-file
            let mut result_buf = BufWriter::new(Vec::new());

            match extension
                .to_ascii_lowercase()
                .to_str()
                .ok_or(anyhow!("Failed to convert extension to str"))?
            {
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

            dep_mut!(PhotoManager, |photo_manager| {
                photo_manager.mark_thumbnail_available(hash, &thumbnail_path);
            });

            //ctx.request_repaint();
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

    pub fn get_photo_adjustments(&self, path: &PathBuf) -> PhotoAdjustments {
        self.photo_database.get_photo_adjustments(path)
    }

    pub fn set_photo_adjustments(&mut self, path: &PathBuf, adjustments: PhotoAdjustments) {
        self.photo_database.set_photo_adjustments(path, adjustments);
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

    pub fn add_to_album(&mut self, album_id: &AlbumId, photo_path: &Path) {
        self.photo_database.add_to_album(album_id, photo_path);
    }

    pub fn remove_from_album(&mut self, album_id: &AlbumId, photo_path: &Path) {
        self.photo_database.remove_from_album(album_id, photo_path);
    }

    pub fn get_photo_albums(&self, photo_path: &Path) -> HashSet<AlbumId> {
        self.photo_database.get_photo_albums(photo_path)
    }

    pub fn get_photo_album(&self, album_id: &AlbumId) -> Option<&Album> {
        self.photo_database.get_photo_album(album_id)
    }

    pub fn create_album(&mut self, album_name: &str) -> Option<AlbumId> {
        self.photo_database.create_album(album_name)
    }

    pub fn insert_album(&mut self, album: Album) -> Option<AlbumId> {
        self.photo_database.insert_album(album)
    }

    pub fn rename_album(&mut self, album_id: &AlbumId, new_name: &str) {
        self.photo_database.rename_album(album_id, new_name);
    }

    pub fn delete_album(&mut self, album_id: &AlbumId) {
        self.photo_database.delete_album(album_id);
    }
}

impl std::fmt::Debug for ContextCache {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ContextCache")
            .field("texture_cache", &self.texture_cache)
            .field(
                "adjusted_texture_cache_len",
                &self.adjusted_texture_cache.len(),
            )
            .field("pending_textures", &self.pending_textures)
            .field(
                "pending_adjusted_textures_len",
                &self.pending_adjusted_textures.len(),
            )
            .field("full_res_accesses", &self.full_res_accesses)
            .field("full_res_access_counter", &self.full_res_access_counter)
            .field(
                "adjusted_thumbnail_accesses",
                &self.adjusted_thumbnail_accesses,
            )
            .field(
                "adjusted_thumbnail_access_counter",
                &self.adjusted_thumbnail_access_counter,
            )
            .finish()
    }
}

struct AdjustedTextureImage {
    size: [usize; 2],
    rgba: Vec<u8>,
}

fn adjusted_texture_cache_key(
    uri: &str,
    source_version: &str,
    adjustments: &PhotoAdjustments,
) -> String {
    format!(
        "adjusted:{}:{}:{}",
        uri,
        source_version,
        hash64(&adjustments.cache_key())
    )
}

fn adjusted_thumbnail_cache_key(
    uri: &str,
    source_version: &str,
    adjustments: &PhotoAdjustments,
) -> String {
    format!(
        "adjusted_thumbnail:{}:{}:{}",
        uri,
        source_version,
        hash64(&adjustments.cache_key())
    )
}

fn adjusted_texture_image(
    source_path: PathBuf,
    adjustments: PhotoAdjustments,
    max_texture_side: u32,
) -> std::result::Result<AdjustedTextureImage, String> {
    let image = decode_oriented_image(&source_path, max_texture_side)?;

    let rgba = adjustments.apply_to_image(image);
    Ok(AdjustedTextureImage {
        size: [rgba.width() as usize, rgba.height() as usize],
        rgba: rgba.into_raw(),
    })
}

fn wait_for_texture<T>(
    description: &str,
    timeout: Duration,
    poll_interval: Duration,
    cancellation: Option<&CancellationToken>,
    mut poll: impl FnMut() -> anyhow::Result<Option<T>>,
) -> anyhow::Result<T> {
    let started_at = Instant::now();

    loop {
        if cancellation.is_some_and(CancellationToken::is_cancelled) {
            return Err(anyhow!("Cancelled loading {description}"));
        }
        match poll() {
            Ok(Some(texture)) => return Ok(texture),
            Ok(None) if started_at.elapsed() < timeout => {
                std::thread::sleep(poll_interval);
            }
            Ok(None) => {
                return Err(anyhow!(
                    "Timed out loading {description} after {:.1} seconds",
                    timeout.as_secs_f32()
                ));
            }
            Err(error) => {
                return Err(anyhow!("Failed to load {description}: {error}"));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::photo::{MetadataCollection, PhotoMetadata};

    fn test_photo_file(label: &str) -> (PathBuf, Photo) {
        let path = std::env::temp_dir().join(format!(
            "photobook-{label}-{}-{}.png",
            std::process::id(),
            rand::random::<u64>()
        ));
        image::RgbaImage::from_pixel(2, 2, image::Rgba([20, 40, 60, 255]))
            .save(&path)
            .unwrap();
        let photo = Photo {
            path: path.clone(),
            metadata: PhotoMetadata {
                fields: MetadataCollection::new(),
            },
            thumbnail_hash: "export-texture-test".to_owned(),
            last_modified: None,
        };
        (path, photo)
    }

    fn texture_test_context() -> Context {
        let ctx = Context::default();
        egui_extras::install_image_loaders(&ctx);
        ctx
    }

    #[test]
    fn export_unadjusted_full_resolution_returns_ready_texture() {
        let (path, photo) = test_photo_file("unadjusted");
        let ctx = texture_test_context();
        let mut manager = PhotoManager::new();

        let result = manager.texture_for_export(&photo, &PhotoAdjustments::default(), &ctx);
        let _ = std::fs::remove_file(path);

        assert!(result.is_ok());
    }

    #[test]
    fn export_adjusted_full_resolution_returns_ready_texture() {
        let (path, photo) = test_photo_file("adjusted");
        let ctx = texture_test_context();
        let mut manager = PhotoManager::new();
        let mut adjustments = PhotoAdjustments::default();
        adjustments.light.exposure = 0.5;

        let result = manager.texture_for_export(&photo, &adjustments, &ctx);
        let _ = std::fs::remove_file(path);

        assert!(result.is_ok());
    }

    #[test]
    fn transferred_context_cache_reuses_adjusted_texture_without_reloading_source() {
        let (path, photo) = test_photo_file("transferred-adjusted");
        let ctx = texture_test_context();
        let mut source_manager = PhotoManager::new();
        let mut destination_manager = PhotoManager::new();
        let mut adjustments = PhotoAdjustments::default();
        adjustments.light.exposure = 0.5;

        let loaded = source_manager
            .texture_for_export(&photo, &adjustments, &ctx)
            .unwrap();
        assert!(
            source_manager.transfer_cached_textures_for_context_to(&ctx, &mut destination_manager)
        );
        assert!(source_manager.caches.is_empty());
        let transferred_cache = destination_manager
            .caches
            .iter()
            .find(|(context, _)| context == &ctx)
            .map(|(_, cache)| cache)
            .unwrap();
        assert!(
            transferred_cache
                .adjusted_texture_cache
                .values()
                .any(|texture| texture.id() == loaded.id)
        );

        let reused = destination_manager
            .texture_for(
                &photo,
                &ctx,
                PhotoTextureOptions::full_resolution().with_adjustments(&adjustments),
            )
            .unwrap()
            .unwrap();
        let _ = std::fs::remove_file(path);

        assert_eq!(reused.id, loaded.id);
    }

    #[test]
    fn export_texture_wait_returns_ready_value_after_pending_polls() {
        let mut poll_count = 0;

        let texture = wait_for_texture(
            "test texture",
            Duration::from_secs(1),
            Duration::ZERO,
            None,
            || {
                poll_count += 1;
                Ok((poll_count == 3).then_some(42))
            },
        )
        .unwrap();

        assert_eq!(texture, 42);
        assert_eq!(poll_count, 3);
    }

    #[test]
    fn export_texture_wait_propagates_loader_errors_with_context() {
        let error = wait_for_texture::<()>(
            "file:///broken.jpg",
            Duration::from_secs(1),
            Duration::ZERO,
            None,
            || Err(anyhow!("decode failed")),
        )
        .unwrap_err();

        let message = error.to_string();
        assert!(message.contains("file:///broken.jpg"));
        assert!(message.contains("decode failed"));
    }

    #[test]
    fn export_texture_wait_honors_cancellation_without_polling() {
        let cancellation = CancellationToken::new();
        cancellation.cancel();
        let mut polled = false;

        let error = wait_for_texture::<()>(
            "cancelled texture",
            Duration::from_secs(1),
            Duration::ZERO,
            Some(&cancellation),
            || {
                polled = true;
                Ok(None)
            },
        )
        .unwrap_err();

        assert!(!polled);
        assert!(error.to_string().contains("Cancelled"));
    }

    #[test]
    fn export_texture_wait_times_out_instead_of_remaining_pending() {
        let error = wait_for_texture::<()>(
            "file:///pending.jpg",
            Duration::ZERO,
            Duration::ZERO,
            None,
            || Ok(None),
        )
        .unwrap_err();

        let message = error.to_string();
        assert!(message.contains("file:///pending.jpg"));
        assert!(message.contains("Timed out"));
    }
}

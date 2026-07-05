use std::{
    collections::{HashMap, HashSet},
    io::BufWriter,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

use glob::MatchOptions;

use chrono::{DateTime, Utc};
use eframe::egui::{ColorImage, Context, TextureHandle, TextureOptions, load::SizedTexture};
use egui::emath::OrderedFloat;
use image::{
    ExtendedColorType,
    codecs::{jpeg::JpegEncoder, png::PngEncoder},
};
use indexmap::IndexMap;
use log::{error, info};
use tokio::sync::oneshot;
use tokio::sync::oneshot::error::TryRecvError;
use tokio::task::spawn_blocking;
use tokio::{fs::File as TokioFile, io::AsyncWriteExt};

use crate::{
    app_status::{AppJob, AppJobStatus, AppStatus},
    dep, dep_mut,
    dirs::Dirs,
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
            let glob_patterns = vec![
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

    pub fn thumbnail_texture_for(
        &mut self,
        photo: &Photo,
        ctx: &Context,
    ) -> anyhow::Result<Option<SizedTexture>> {
        if !self.thumbnail_exists(photo) {
            return Ok(None);
        }

        let adjustments = self.get_photo_adjustments(&photo.path);
        let cache = self.get_cache_mut(ctx);
        if adjustments.is_identity() {
            Self::load_texture(
                &photo.thumbnail_uri(),
                ctx,
                &mut cache.texture_cache,
                &mut cache.pending_textures,
            )
        } else {
            Self::load_adjusted_thumbnail_texture(photo, &adjustments, ctx, cache)
        }
    }

    pub fn unadjusted_thumbnail_texture_for(
        &mut self,
        photo: &Photo,
        ctx: &Context,
    ) -> anyhow::Result<Option<SizedTexture>> {
        if !self.thumbnail_exists(photo) {
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
                self.thumbnail_texture_for(&photo, ctx)
            }
            _ => Ok(None),
        }
    }

    pub fn texture_for(
        &mut self,
        photo: &Photo,
        ctx: &Context,
    ) -> anyhow::Result<Option<SizedTexture>> {
        let uri = photo.uri();
        let adjustments = self.get_photo_adjustments(&photo.path);
        let cache = self.get_cache_mut(ctx);
        let protected_uri = if adjustments.is_identity() {
            uri.clone()
        } else {
            adjusted_texture_cache_key(&uri, &path_version_key(&photo.path), &adjustments)
        };
        let result = if adjustments.is_identity() {
            Self::load_full_res_texture(&uri, ctx, cache)
        } else {
            Self::load_adjusted_full_res_texture(photo, &adjustments, ctx, cache)
        };

        Self::evict_full_res_textures(ctx, cache, &protected_uri);

        result
    }

    pub fn texture_for_blocking(
        &mut self,
        photo: &Photo,
        ctx: &Context,
    ) -> anyhow::Result<Option<SizedTexture>> {
        let uri = photo.uri();
        let adjustments = self.get_photo_adjustments(&photo.path);
        let cache = self.get_cache_mut(ctx);
        let protected_uri = if adjustments.is_identity() {
            uri.clone()
        } else {
            adjusted_texture_cache_key(&uri, &path_version_key(&photo.path), &adjustments)
        };
        let result = if adjustments.is_identity() {
            Self::load_full_res_texture_blocking(&uri, ctx, cache)
        } else {
            Self::load_adjusted_full_res_texture(photo, &adjustments, ctx, cache)
        };

        Self::evict_full_res_textures(ctx, cache, &protected_uri);

        result
    }

    pub fn texture_for_photo_with_thumbail_backup(
        &mut self,
        photo: &Photo,
        ctx: &Context,
    ) -> anyhow::Result<Option<SizedTexture>> {
        let uri = photo.uri();
        let adjustments = self.get_photo_adjustments(&photo.path);
        let thumbnail_exists = self.thumbnail_exists(photo);
        let cache = self.get_cache_mut(ctx);
        let protected_uri = if adjustments.is_identity() {
            uri.clone()
        } else {
            adjusted_texture_cache_key(&uri, &path_version_key(&photo.path), &adjustments)
        };
        let full_res_result = if adjustments.is_identity() {
            Self::load_full_res_texture(&uri, ctx, cache)
        } else {
            Self::load_adjusted_full_res_texture(photo, &adjustments, ctx, cache)
        };
        let result = match full_res_result {
            Ok(Some(tex)) => Ok(Some(tex)),
            _ if !thumbnail_exists => Ok(None),
            _ if adjustments.is_identity() => Self::load_texture(
                &photo.thumbnail_uri(),
                ctx,
                &mut cache.texture_cache,
                &mut cache.pending_textures,
            ),
            _ => Self::load_adjusted_thumbnail_texture(photo, &adjustments, ctx, cache),
        };

        Self::evict_full_res_textures(ctx, cache, &protected_uri);

        result
    }

    #[allow(dead_code)]
    pub fn texture_at(&mut self, at: usize, ctx: &Context) -> anyhow::Result<Option<SizedTexture>> {
        match self.photo_database.get_photo_by_index(at) {
            Some(photo) => {
                let photo = photo.clone();
                let uri = photo.uri();
                let cache = self.get_cache_mut(ctx);
                let result = Self::load_full_res_texture(&uri, ctx, cache);

                Self::evict_full_res_textures(ctx, cache, &uri);

                result
            }
            _ => Ok(None),
        }
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

    fn load_adjusted_full_res_texture(
        photo: &Photo,
        adjustments: &PhotoAdjustments,
        ctx: &Context,
        cache: &mut ContextCache,
    ) -> anyhow::Result<Option<SizedTexture>> {
        let uri = photo.uri();
        let source_version = path_version_key(&photo.path);
        let cache_key = adjusted_texture_cache_key(&uri, &source_version, adjustments);
        Self::touch_full_res_texture(cache, &cache_key);

        let result = Self::load_adjusted_texture(
            photo.path.clone(),
            cache_key.clone(),
            adjustments,
            ctx.input(|input| input.max_texture_side as u32),
            !ctx.input(|input| input.pointer.primary_down()),
            ctx,
            cache,
        );

        if result.is_err() {
            cache.full_res_accesses.remove(&cache_key);
        }

        result
    }

    fn load_adjusted_thumbnail_texture(
        photo: &Photo,
        adjustments: &PhotoAdjustments,
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
            true,
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
            ctx.request_repaint();
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
        );
        cache.pending_adjusted_textures.insert(cache_key, receiver);
        ctx.request_repaint();

        Ok(None)
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
    ) {
        let load = move || adjusted_texture_image(source_path, adjustments, max_texture_side);

        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(async move {
                let result = spawn_blocking(load)
                    .await
                    .map_err(|error| error.to_string())
                    .and_then(|result| result);
                let _ = sender.send(result);
            });
        } else {
            std::thread::spawn(move || {
                let _ = sender.send(load());
            });
        }
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
                        Err(anyhow!(err))
                    }
                }
            }
        }
    }

    fn gen_thumbnails(photo_paths: Vec<PathBuf>) -> anyhow::Result<()> {
        if photo_paths.len() > 0 {
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

    async fn gen_thumbnail(photo_path: &PathBuf, thumbnail_dir: &PathBuf) -> anyhow::Result<()> {
        let file_name = photo_path.file_name();
        let extension = photo_path.extension();

        if let (Some(_), Some(extension)) = (file_name, extension) {
            if extension.to_ascii_lowercase() == "jpg"
                || extension.to_ascii_lowercase() == "png"
                || extension.to_ascii_lowercase() == "jpeg"
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

fn path_version_key(path: &Path) -> String {
    let Ok(metadata) = path.metadata() else {
        return "missing".to_owned();
    };

    let modified = metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();

    format!("{}:{}", metadata.len(), modified)
}

fn adjusted_texture_image(
    source_path: PathBuf,
    adjustments: PhotoAdjustments,
    max_texture_side: u32,
) -> std::result::Result<AdjustedTextureImage, String> {
    let mut image = image::open(&source_path).map_err(|error| error.to_string())?;
    if max_texture_side > 0
        && (image.width() > max_texture_side || image.height() > max_texture_side)
    {
        image = image.resize(
            max_texture_side,
            max_texture_side,
            image::imageops::FilterType::Triangle,
        );
    }

    let rgba = adjustments.apply_to_image(image);
    Ok(AdjustedTextureImage {
        size: [rgba.width() as usize, rgba.height() as usize],
        rgba: rgba.into_raw(),
    })
}

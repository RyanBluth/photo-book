use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

use fxhash::hash64;
use tokio::sync::oneshot;
use tokio::sync::oneshot::error::TryRecvError;
use tokio::task::spawn_blocking;

use crate::{
    dirs::Dirs, model::photo_adjustments::PhotoAdjustments, widget::histogram::HistogramData,
};

const HISTOGRAM_CACHE_CAPACITY: usize = 96;
const HISTOGRAM_VERSION: u32 = 1;

#[derive(Debug)]
pub enum HistogramLoadResult {
    Ready(HistogramData),
    Pending(Option<HistogramData>),
    Unavailable(String, Option<HistogramData>),
}

impl HistogramLoadResult {
    pub fn ready_data(&self) -> Option<&HistogramData> {
        match self {
            Self::Ready(data) => Some(data),
            Self::Pending(data) | Self::Unavailable(_, data) => data.as_ref(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct HistogramKey {
    photo_path: PathBuf,
    thumbnail_path: PathBuf,
    thumbnail_version: String,
    adjustments_key: String,
    histogram_version: u32,
}

#[derive(Debug)]
struct HistogramCacheEntry {
    result: Result<HistogramData, String>,
    last_access: u64,
}

#[derive(Debug, Default)]
pub struct HistogramManager {
    cache: HashMap<HistogramKey, HistogramCacheEntry>,
    pending: HashMap<HistogramKey, oneshot::Receiver<Result<HistogramData, String>>>,
    last_ready_by_photo: HashMap<PathBuf, HistogramData>,
    access_counter: u64,
}

impl HistogramManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(
        &mut self,
        path: &Path,
        adjustments: &PhotoAdjustments,
        refresh: bool,
    ) -> HistogramLoadResult {
        self.access_counter = self.access_counter.saturating_add(1);
        let access = self.access_counter;
        let thumbnail_path = thumbnail_path_for_photo(path);
        let fallback = self.last_ready_by_photo.get(path).cloned();

        if !thumbnail_path.exists() {
            return HistogramLoadResult::Unavailable(
                "Histogram unavailable: thumbnail is pending".to_owned(),
                fallback,
            );
        }

        let key = HistogramKey {
            photo_path: path.to_path_buf(),
            thumbnail_path: thumbnail_path.clone(),
            thumbnail_version: path_version_key(&thumbnail_path),
            adjustments_key: adjustments.cache_key(),
            histogram_version: HISTOGRAM_VERSION,
        };

        if let Some(entry) = self.cache.get_mut(&key) {
            entry.last_access = access;
            return match &entry.result {
                Ok(data) => HistogramLoadResult::Ready(data.clone()),
                Err(error) => HistogramLoadResult::Unavailable(error.clone(), fallback),
            };
        }

        if let Some(result) = self.poll_pending(&key) {
            let result = result.map_err(|error| format!("Histogram unavailable: {error}"));
            let response = match &result {
                Ok(data) => {
                    self.last_ready_by_photo
                        .insert(path.to_path_buf(), data.clone());
                    HistogramLoadResult::Ready(data.clone())
                }
                Err(error) => HistogramLoadResult::Unavailable(error.clone(), fallback),
            };

            self.cache.insert(
                key.clone(),
                HistogramCacheEntry {
                    result,
                    last_access: access,
                },
            );
            self.evict_old_entries(&key);
            return response;
        }

        if !refresh {
            return HistogramLoadResult::Pending(fallback);
        }

        if !self.pending.contains_key(&key) {
            let (sender, receiver) = oneshot::channel();
            spawn_histogram_job(thumbnail_path, adjustments.clone(), sender);
            self.pending.insert(key, receiver);
        }

        HistogramLoadResult::Pending(fallback)
    }

    fn poll_pending(&mut self, key: &HistogramKey) -> Option<Result<HistogramData, String>> {
        let result = match self.pending.get_mut(key) {
            Some(receiver) => match receiver.try_recv() {
                Ok(result) => Some(result),
                Err(TryRecvError::Empty) => None,
                Err(TryRecvError::Closed) => Some(Err("histogram worker stopped".to_owned())),
            },
            None => None,
        };

        if result.is_some() {
            self.pending.remove(key);
        }

        result
    }

    fn evict_old_entries(&mut self, protected_key: &HistogramKey) {
        let capacity = HISTOGRAM_CACHE_CAPACITY.max(1);
        while self.cache.len() > capacity {
            let Some(key) = self
                .cache
                .iter()
                .filter(|(key, _)| *key != protected_key)
                .min_by_key(|(_, entry)| entry.last_access)
                .map(|(key, _)| key.clone())
            else {
                break;
            };

            self.cache.remove(&key);
        }
    }
}

pub(crate) fn thumbnail_path_for_photo(path: &Path) -> PathBuf {
    Dirs::Thumbnails
        .path()
        .join(hash64(&path.to_string_lossy()).to_string())
        .with_extension(path.extension().unwrap_or_default())
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

fn spawn_histogram_job(
    thumbnail_path: PathBuf,
    adjustments: PhotoAdjustments,
    sender: oneshot::Sender<Result<HistogramData, String>>,
) {
    let compute = move || compute_histogram(thumbnail_path, adjustments);

    if let Ok(handle) = tokio::runtime::Handle::try_current() {
        handle.spawn(async move {
            let result = spawn_blocking(compute)
                .await
                .map_err(|error| error.to_string())
                .and_then(|result| result);
            let _ = sender.send(result);
        });
    } else {
        std::thread::spawn(move || {
            let _ = sender.send(compute());
        });
    }
}

fn compute_histogram(
    thumbnail_path: PathBuf,
    adjustments: PhotoAdjustments,
) -> Result<HistogramData, String> {
    let image = image::open(thumbnail_path).map_err(|error| error.to_string())?;
    Ok(HistogramData::from_image_with_adjustments(
        &image,
        &adjustments,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgba, RgbaImage};

    #[test]
    fn histogram_manager_retries_missing_thumbnail() {
        let path = test_photo_path("retry");
        let thumbnail_path = thumbnail_path_for_photo(&path);
        remove_test_thumbnail(thumbnail_path.clone());

        let mut manager = HistogramManager::new();
        assert!(matches!(
            manager.get(&path, &PhotoAdjustments::default(), true),
            HistogramLoadResult::Unavailable(_, None)
        ));

        let image = RgbaImage::from_pixel(8, 8, Rgba([72, 72, 72, 255]));
        write_test_thumbnail(&path, &image);

        assert!(matches!(
            manager.get(&path, &PhotoAdjustments::default(), true),
            HistogramLoadResult::Pending(None)
        ));

        for _ in 0..100 {
            if matches!(
                manager.get(&path, &PhotoAdjustments::default(), true),
                HistogramLoadResult::Ready(_)
            ) {
                remove_test_thumbnail(thumbnail_path);
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }

        remove_test_thumbnail(thumbnail_path);
        panic!("histogram did not become ready");
    }

    #[test]
    fn histogram_manager_keeps_previous_ready_result_while_refresh_is_deferred() {
        let path = test_photo_path("deferred");
        let image = RgbaImage::from_pixel(8, 8, Rgba([32, 32, 32, 255]));
        let thumbnail_path = write_test_thumbnail(&path, &image);

        let mut manager = HistogramManager::new();
        wait_for_ready(&mut manager, &path, &PhotoAdjustments::default());

        let mut adjustments = PhotoAdjustments::default();
        adjustments.light.exposure = 2.0;
        let result = manager.get(&path, &adjustments, false);

        assert!(matches!(result, HistogramLoadResult::Pending(Some(_))));
        remove_test_thumbnail(thumbnail_path);
    }

    fn wait_for_ready(
        manager: &mut HistogramManager,
        path: &Path,
        adjustments: &PhotoAdjustments,
    ) -> HistogramData {
        for _ in 0..100 {
            if let HistogramLoadResult::Ready(data) = manager.get(path, adjustments, true) {
                return data;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }

        panic!("histogram did not become ready");
    }

    fn test_photo_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "photo_book_histogram_manager_{name}_{}.png",
            std::process::id()
        ))
    }

    fn write_test_thumbnail(photo_path: &Path, image: &RgbaImage) -> PathBuf {
        let thumbnail_path = thumbnail_path_for_photo(photo_path);
        if let Some(parent) = thumbnail_path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        image.save(&thumbnail_path).unwrap();
        thumbnail_path
    }

    fn remove_test_thumbnail(path: PathBuf) {
        let _ = std::fs::remove_file(path);
    }
}

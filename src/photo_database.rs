use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
    hash::Hash,
    path::PathBuf,
    sync::Arc,
};

use chrono::{DateTime, Datelike, Utc};
use indexmap::IndexMap;

use crate::{
    file_tree::FileTreeCollection,
    id::next_query_result_id,
    model::photo_grouping::PhotoGrouping,
    photo::{Photo, PhotoMetadataField, PhotoMetadataFieldLabel, PhotoRating},
};

#[derive(Debug, Clone)]
struct BidirectionalHashMap<K, V> {
    forward: HashMap<K, V>,
    backward: HashMap<V, K>,
}

impl<K: Hash + Eq + Clone, V: Hash + Eq + Clone> BidirectionalHashMap<K, V> {
    pub fn new() -> Self {
        Self {
            forward: HashMap::new(),
            backward: HashMap::new(),
        }
    }

    pub fn clear(&mut self) {
        self.forward.clear();
        self.backward.clear();
    }

    pub fn insert(&mut self, key: K, value: V) {
        self.forward.insert(key.clone(), value.clone());
        self.backward.insert(value.clone(), key.clone());
    }

    pub fn get_left(&self, key: &K) -> Option<&V> {
        self.forward.get(key)
    }

    pub fn get_left_mut(&mut self, key: &K) -> Option<&mut V> {
        self.forward.get_mut(key)
    }

    pub fn get_right(&self, value: &V) -> Option<&K> {
        self.backward.get(value)
    }

    pub fn get_right_mut(&mut self, value: &V) -> Option<&mut K> {
        self.backward.get_mut(value)
    }

    pub fn remove_left(&mut self, key: &K) -> Option<V> {
        if let Some(value) = self.forward.remove(key) {
            self.backward.remove(&value);
            Some(value)
        } else {
            None
        }
    }

    pub fn remove_right(&mut self, value: &V) -> Option<K> {
        if let Some(key) = self.backward.remove(value) {
            self.forward.remove(&key);
            Some(key)
        } else {
            None
        }
    }
}

#[derive(Debug, Clone)]
pub struct PhotoDatabase {
    photos: Vec<Photo>,
    path_map: BidirectionalHashMap<usize, PathBuf>,
    photo_ratings: HashMap<PathBuf, PhotoRating>,
    photo_tags: HashMap<PathBuf, HashSet<String>>,
    query_cache: HashMap<PhotoQuery, PhotoQueryResult>,
    pub file_collection: FileTreeCollection,
    is_sorted: bool,
}

impl PhotoDatabase {
    pub fn new() -> Self {
        Self {
            photos: Vec::new(),
            path_map: BidirectionalHashMap::new(),
            photo_ratings: HashMap::new(),
            photo_tags: HashMap::new(),
            query_cache: HashMap::new(),
            file_collection: FileTreeCollection::new(),
            is_sorted: true,
        }
    }

    pub fn has_photos(&self) -> bool {
        !self.photos.is_empty()
    }

    pub fn clear(&mut self) {
        self.photos.clear();
        self.path_map.clear();
        self.photo_ratings.clear();
        self.photo_tags.clear();
        self.query_cache.clear();
        self.file_collection = FileTreeCollection::new();
        self.is_sorted = true;
    }

    pub fn add_photo(&mut self, photo: Photo) {
        let path = photo.path.clone();
        self.path_map.insert(self.photos.len(), path.clone());
        self.file_collection.insert(&path);
        self.photos.push(photo);
        self.is_sorted = false;
        self.invalidate_query_cache();
    }

    pub fn get_photo(&self, path: &PathBuf) -> Option<&Photo> {
        self.path_map
            .get_right(path)
            .map(|index| &self.photos[*index])
    }

    pub fn get_photo_mut(&mut self, path: &PathBuf) -> Option<&mut Photo> {
        self.path_map
            .get_right(path)
            .map(|index| &mut self.photos[*index])
    }

    pub fn get_photo_by_index(&mut self, index: usize) -> Option<&Photo> {
        self.ensure_sorted();
        self.photos.get(index)
    }

    pub fn get_photo_by_index_mut(&mut self, index: usize) -> Option<&mut Photo> {
        self.ensure_sorted();
        self.photos.get_mut(index)
    }

    pub fn remove_photo(&mut self, path: &PathBuf) {
        if let Some(index) = self.path_map.remove_right(path) {
            // If we're not removing the last element, we need to update the path_map
            // because swap_remove will move the last element to this index
            let last_index = self.photos.len() - 1;
            if index < last_index {
                // Get the path of the photo that will be moved to this index
                if let Some(moved_photo_path) = self.path_map.get_left(&last_index).cloned() {
                    // Remove the old mapping for the last element
                    self.path_map.remove_left(&last_index);
                    // Insert the new mapping with the updated index
                    self.path_map.insert(index, moved_photo_path);
                }
            }
            self.photos.swap_remove(index);
        }
        self.photo_ratings.remove(path);
        self.photo_tags.remove(path);
        self.file_collection.remove(path);
        self.is_sorted = false;
        self.invalidate_query_cache();
    }

    pub fn invalidate_query_cache(&mut self) {
        self.query_cache.clear();
    }

    pub fn query_photos(&mut self, query: &PhotoQuery) -> PhotoQueryResult {
        self.ensure_sorted();

        if let Some(cached) = self.query_cache.get(query) {
            return cached.clone();
        }

        let mut result: HashSet<Photo> = self.photos.clone().into_iter().collect();

        if let Some(query_ratings) = &query.ratings {
            let mut rating_set: HashSet<Photo> = HashSet::new();
            for (path, rating) in &self.photo_ratings {
                if query_ratings.contains(rating) {
                    if let Some(photo) = self.get_photo(path) {
                        rating_set.insert(photo.clone());
                    }
                }
            }
            result = result.intersection(&rating_set).cloned().collect();
        }

        if let Some(query_tags) = &query.tags {
            for tag in query_tags {
                let mut tag_set: HashSet<Photo> = HashSet::new();
                for (path, tags) in &self.photo_tags {
                    if tags.contains(tag) {
                        if let Some(photo) = self.get_photo(path) {
                            tag_set.insert(photo.clone());
                        }
                    }
                }
                result = result.intersection(&tag_set).cloned().collect();
            }
        }

        // Sort by date
        let mut sorted_result = result.into_iter().collect::<Vec<_>>();
        sorted_result.sort_by(Self::compare_photos);

        let mut grouped_photos: IndexMap<String, IndexMap<PathBuf, Photo>> = IndexMap::new();
        for photo in &sorted_result {
            match query.grouping {
                PhotoGrouping::Rating => {
                    let rating = self.get_photo_rating(&photo.path);
                    let group_label = format!("{:?}", rating);
                    grouped_photos
                        .entry(group_label)
                        .or_insert_with(IndexMap::new)
                        .insert(photo.path.clone(), photo.clone());
                }
                PhotoGrouping::Tag => {
                    let tags = self.get_photo_tags(&photo.path);
                    let group_label = format!("{:?}", tags);
                    grouped_photos
                        .entry(group_label)
                        .or_insert_with(IndexMap::new)
                        .insert(photo.path.clone(), photo.clone());
                }
                PhotoGrouping::Date => {
                    let date_time = photo
                        .metadata
                        .fields
                        .get(PhotoMetadataFieldLabel::DateTime)
                        .and_then(|field| match field {
                            PhotoMetadataField::DateTime(date_time) => Some(date_time.clone()),
                            _ => {
                                if let Result::Ok(file_metadata) =
                                    std::fs::metadata(photo.path.clone())
                                {
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

                    let key = if let Some(date_time) = date_time {
                        let year = date_time.year();
                        let month = date_time.month();
                        let day = date_time.day();

                        format!("{:04}-{:02}-{:02}", year, month, day)
                    } else {
                        "Unknown Date".to_string()
                    };
                    grouped_photos
                        .entry(key)
                        .or_insert_with(IndexMap::new)
                        .insert(photo.path.clone(), photo.clone());
                }
            }
        }

        let result = PhotoQueryResult::new(grouped_photos);
        self.query_cache.insert(query.clone(), result.clone());
        result
    }

    fn index_of_photo(&self, path: &PathBuf) -> Option<usize> {
        self.path_map.get_right(path).copied()
    }

    pub fn get_photo_rating(&self, path: &PathBuf) -> PhotoRating {
        self.photo_ratings.get(path).cloned().unwrap_or_default()
    }

    pub fn set_photo_rating(&mut self, path: &PathBuf, rating: PhotoRating) {
        if self.photo_ratings.get(path) == Some(&rating) {
            return;
        }
        self.photo_ratings.insert(path.clone(), rating);
        self.invalidate_query_cache();
    }

    pub fn get_photo_tags(&self, path: &PathBuf) -> HashSet<String> {
        self.photo_tags.get(path).cloned().unwrap_or_default()
    }

    pub fn set_photo_tags(&mut self, path: &PathBuf, tags: HashSet<String>) {
        if self.photo_tags.get(path) == Some(&tags) {
            return;
        }
        self.photo_tags.insert(path.clone(), tags);
        self.invalidate_query_cache();
    }

    pub fn add_photo_tag(&mut self, path: &PathBuf, tag: String) {
        if let Some(tags) = self.photo_tags.get(path) {
            if tags.contains(&tag) {
                return;
            }
        }
        self.photo_tags.entry(path.clone()).or_default().insert(tag);
        self.invalidate_query_cache();
    }

    pub fn remove_photo_tag(&mut self, path: &PathBuf, tag: &String) {
        if let Some(tags) = self.photo_tags.get_mut(path) {
            if !tags.contains(tag) {
                return;
            }
            tags.remove(tag);
        }
        self.invalidate_query_cache();
    }

    pub fn all_tags(&self) -> Vec<String> {
        let mut tags = HashSet::new();
        for photo_tags in self.photo_tags.values() {
            for tag in photo_tags {
                tags.insert(tag.clone());
            }
        }
        let mut sorted_tags: Vec<String> = tags.into_iter().collect();
        sorted_tags.sort();
        sorted_tags
    }

    /// Check if a photo exists in the database
    pub fn photo_exists(&self, path: &PathBuf) -> bool {
        self.path_map.get_right(path).is_some()
    }

    /// Get the number of photos in the database
    pub fn photo_count(&self) -> usize {
        self.photos.len()
    }

    /// Update an existing photo in the database
    pub fn update_photo(&mut self, photo: Photo) {
        if let Some(index) = self.path_map.get_right(&photo.path) {
            self.photos[*index] = photo;
            self.invalidate_query_cache();
        }
    }

    /// Get photo by index with sorting applied
    pub fn get_photo_by_index_sorted(&self, index: usize) -> Option<&Photo> {
        // For now, just return by index - sorting will be handled by PhotoManager
        self.photos.get(index)
    }

    /// Get the index of a photo by its path
    pub fn get_photo_index(&mut self, path: &PathBuf) -> Option<usize> {
        self.ensure_sorted();
        self.path_map.get_right(path).copied()
    }

    /// Get all photo paths
    pub fn get_all_photo_paths(&self) -> Vec<PathBuf> {
        self.path_map.backward.keys().cloned().collect()
    }

    /// Get flattened file trees (for UI display)
    pub fn get_flattened_file_trees(&mut self) -> Arc<Vec<crate::file_tree::FlattenedTreeItem>> {
        self.file_collection.flattened_file_trees()
    }

    /// Ensure photos are sorted (internal use)
    fn ensure_sorted(&mut self) {
        if !self.is_sorted {
            self.sort_photos(PhotoSortCriteria::Date);
        }
    }

    /// Sort photos by a given criteria (modifies internal order)
    pub fn sort_photos(&mut self, criteria: PhotoSortCriteria) {
        match criteria {
            PhotoSortCriteria::Date => {
                // Create a vector of (index, photo) pairs
                let mut indexed_photos: Vec<(usize, Photo)> = self
                    .photos
                    .iter()
                    .enumerate()
                    .map(|(i, photo)| (i, photo.clone()))
                    .collect();

                // Sort by the photo comparison function
                indexed_photos.sort_by(|a, b| Self::compare_photos(&a.1, &b.1));

                // Rebuild the photos vector and update the path_map
                let mut new_photos = Vec::new();
                let mut new_path_map = BidirectionalHashMap::new();

                for (new_index, (_, photo)) in indexed_photos.into_iter().enumerate() {
                    new_path_map.insert(new_index, photo.path.clone());
                    new_photos.push(photo);
                }

                self.photos = new_photos;
                self.path_map = new_path_map;
                self.is_sorted = true;
                self.invalidate_query_cache();
            }
        }
    }

    fn compare_photos(a: &Photo, b: &Photo) -> Ordering {
        match (
            a.metadata.fields.get(PhotoMetadataFieldLabel::DateTime),
            b.metadata.fields.get(PhotoMetadataFieldLabel::DateTime),
        ) {
            (Some(PhotoMetadataField::DateTime(a)), Some(PhotoMetadataField::DateTime(b))) => {
                b.timestamp().cmp(&a.timestamp())
            }
            (Some(PhotoMetadataField::DateTime(_)), None) => Ordering::Greater,
            (None, Some(PhotoMetadataField::DateTime(_))) => Ordering::Less,
            _ => a.path.cmp(&b.path),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PhotoSortCriteria {
    Date,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PhotoQuery {
    pub ratings: Option<Vec<PhotoRating>>,
    pub tags: Option<Vec<String>>,
    pub grouping: PhotoGrouping,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhotoQueryResult {
    pub groups: IndexMap<String, IndexMap<PathBuf, Photo>>,
    id: usize,
    path_to_index_map: HashMap<String, (usize, usize)>,
}

impl PhotoQueryResult {
    pub fn new(groups: IndexMap<String, IndexMap<PathBuf, Photo>>) -> Self {
        let mut path_to_index_map = HashMap::new();

        for group_idx in 0..groups.len() {
            if let Some((_group_name, photos)) = groups.get_index(group_idx) {
                for photo_idx in 0..photos.len() {
                    if let Some((_path, photo)) = photos.get_index(photo_idx) {
                        path_to_index_map.insert(photo.string_path(), (group_idx, photo_idx));
                    }
                }
            }
        }

        Self {
            groups,
            id: next_query_result_id(),
            path_to_index_map,
        }
    }

    pub fn id(&self) -> usize {
        self.id
    }

    pub fn photo_after(&self, photo: &Photo) -> Option<Photo> {
        let (group_idx, photo_idx) = self.path_to_index_map.get(&photo.string_path())?;
        let (_group_name, photos) = self.groups.get_index(*group_idx)?;

        if photo_idx + 1 < photos.len() {
            let (_group, next_photo) = photos.get_index(photo_idx + 1)?;
            return Some(next_photo.clone());
        }

        if group_idx + 1 < self.groups.len() {
            let (_, next_photos) = self.groups.get_index(group_idx + 1)?;
            let (_group, next_photo) = next_photos.get_index(0)?;
            return Some(next_photo.clone());
        }

        return None;
    }

    pub fn photo_before(&self, photo: &Photo) -> Option<Photo> {
        let (group_idx, photo_idx) = self.path_to_index_map.get(&photo.string_path())?;
        let (_group_name, photos) = self.groups.get_index(*group_idx)?;

        if *photo_idx > 0 {
            let (_group, prev_photo) = photos.get_index(photo_idx - 1)?;
            return Some(prev_photo.clone());
        }

        if *group_idx > 0 {
            let (_, prev_photos) = self.groups.get_index(group_idx - 1)?;
            let (_group, prev_photo) = prev_photos.get_index(prev_photos.len() - 1)?;
            return Some(prev_photo.clone());
        }

        return None;
    }

    fn group_for_photo(&self, photo: &Photo) -> Option<String> {
        let (group_idx, _) = self.path_to_index_map.get(&photo.string_path())?;
        self.groups
            .get_index(*group_idx)
            .map(|(name, _)| name.clone())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhotoQueryResultIterator {
    pub query_result: PhotoQueryResult,
    current_photo: Option<Photo>,
}

impl Iterator for PhotoQueryResultIterator {
    type Item = (String, Photo);

    fn next(&mut self) -> Option<Self::Item> {
        let current = self.current_photo.clone()?;
        let group = self.query_result.group_for_photo(&current)?;

        self.current_photo = self.query_result.photo_after(&current);

        Some((group, current))
    }
}

impl IntoIterator for PhotoQueryResult {
    type Item = (String, Photo);
    type IntoIter = PhotoQueryResultIterator;

    fn into_iter(self) -> Self::IntoIter {
        let current_photo = self
            .groups
            .iter()
            .find_map(|(_, photos)| photos.first().map(|(_, photo)| photo.clone()));

        PhotoQueryResultIterator {
            query_result: self,
            current_photo,
        }
    }
}

impl Default for PhotoQuery {
    fn default() -> Self {
        Self {
            ratings: None,
            tags: None,
            grouping: PhotoGrouping::Date,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::photo::{PhotoMetadata, PhotoMetadataField, PhotoMetadataFieldLabel};
    use chrono::{DateTime, Utc};
    use std::path::PathBuf;

    fn create_test_photo(path: &str, datetime: Option<DateTime<Utc>>) -> Photo {
        let mut metadata = PhotoMetadata {
            fields: crate::photo::MetadataCollection::new(),
        };

        if let Some(dt) = datetime {
            metadata.fields.insert(PhotoMetadataField::DateTime(dt));
        }

        Photo {
            path: PathBuf::from(path),
            metadata,
            thumbnail_hash: "test_hash".to_string(),
        }
    }

    #[test]
    fn test_new_database() {
        let db = PhotoDatabase::new();
        assert_eq!(db.photos.len(), 0);
        assert_eq!(db.query_cache.len(), 0);
    }

    #[test]
    fn test_add_photo() {
        let mut db = PhotoDatabase::new();
        let photo = create_test_photo("/test/photo1.jpg", None);
        let path = photo.path.clone();

        db.add_photo(photo);

        assert_eq!(db.photos.len(), 1);
        assert!(db.get_photo(&path).is_some());
        assert_eq!(db.get_photo_rating(&path), PhotoRating::default());
        assert_eq!(db.get_photo_tags(&path), HashSet::new());
    }

    #[test]
    fn test_get_photo() {
        let mut db = PhotoDatabase::new();
        let photo = create_test_photo("/test/photo1.jpg", None);
        let path = photo.path.clone();

        db.add_photo(photo);

        let retrieved = db.get_photo(&path);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().path, path);

        let non_existent = db.get_photo(&PathBuf::from("/test/nonexistent.jpg"));
        assert!(non_existent.is_none());
    }

    #[test]
    fn test_get_photo_by_index() {
        let mut db = PhotoDatabase::new();
        let photo1 = create_test_photo("/test/photo1.jpg", None);
        let photo2 = create_test_photo("/test/photo2.jpg", None);

        db.add_photo(photo1);
        db.add_photo(photo2);

        assert!(db.get_photo_by_index(0).is_some());
        assert!(db.get_photo_by_index(1).is_some());
        assert!(db.get_photo_by_index(2).is_none());
    }

    #[test]
    fn test_remove_photo() {
        let mut db = PhotoDatabase::new();
        let photo = create_test_photo("/test/photo1.jpg", None);
        let path = photo.path.clone();

        db.add_photo(photo);
        assert_eq!(db.photos.len(), 1);

        db.remove_photo(&path);
        assert_eq!(db.photos.len(), 0);
        assert!(db.get_photo(&path).is_none());
        assert!(!db.photo_ratings.contains_key(&path));
        assert!(!db.photo_tags.contains_key(&path));
    }

    #[test]
    fn test_remove_photo_with_index_handling() {
        let mut db = PhotoDatabase::new();

        // Add multiple photos
        let photo1 = create_test_photo("/test/photo1.jpg", None);
        let photo2 = create_test_photo("/test/photo2.jpg", None);
        let photo3 = create_test_photo("/test/photo3.jpg", None);

        let path1 = photo1.path.clone();
        let path2 = photo2.path.clone();
        let path3 = photo3.path.clone();

        db.add_photo(photo1);
        db.add_photo(photo2);
        db.add_photo(photo3);

        assert_eq!(db.photos.len(), 3);

        // Remove the middle photo (this triggers the index update logic)
        db.remove_photo(&path2);

        assert_eq!(db.photos.len(), 2);
        assert!(db.get_photo(&path1).is_some());
        assert!(db.get_photo(&path2).is_none());
        assert!(db.get_photo(&path3).is_some());

        // Verify that we can still access the remaining photos by index
        assert!(db.get_photo_by_index(0).is_some());
        assert!(db.get_photo_by_index(1).is_some());
        assert!(db.get_photo_by_index(2).is_none());

        // Remove the first photo
        db.remove_photo(&path1);

        assert_eq!(db.photos.len(), 1);
        assert!(db.get_photo(&path1).is_none());
        assert!(db.get_photo(&path3).is_some());

        // Remove the last photo
        db.remove_photo(&path3);

        assert_eq!(db.photos.len(), 0);
        assert!(db.get_photo(&path3).is_none());
    }

    #[test]
    fn test_photo_ratings() {
        let mut db = PhotoDatabase::new();
        let photo = create_test_photo("/test/photo1.jpg", None);
        let path = photo.path.clone();

        db.add_photo(photo);

        // Test default rating
        assert_eq!(db.get_photo_rating(&path), PhotoRating::default());

        // Test setting rating
        db.set_photo_rating(&path, PhotoRating::Yes);
        assert_eq!(db.get_photo_rating(&path), PhotoRating::Yes);

        // Test non-existent photo
        let non_existent = PathBuf::from("/test/nonexistent.jpg");
        assert_eq!(db.get_photo_rating(&non_existent), PhotoRating::default());
    }

    #[test]
    fn test_photo_tags() {
        let mut db = PhotoDatabase::new();
        let photo = create_test_photo("/test/photo1.jpg", None);
        let path = photo.path.clone();

        db.add_photo(photo);

        // Test default tags
        assert_eq!(db.get_photo_tags(&path), HashSet::new());

        // Test adding tags
        db.add_photo_tag(&path, "landscape".to_string());
        db.add_photo_tag(&path, "sunset".to_string());

        let tags = db.get_photo_tags(&path);
        assert!(tags.contains("landscape"));
        assert!(tags.contains("sunset"));
        assert_eq!(tags.len(), 2);

        // Test removing tag
        db.remove_photo_tag(&path, &"landscape".to_string());
        let tags = db.get_photo_tags(&path);
        assert!(!tags.contains("landscape"));
        assert!(tags.contains("sunset"));
        assert_eq!(tags.len(), 1);

        // Test setting tags
        let mut new_tags = HashSet::new();
        new_tags.insert("nature".to_string());
        new_tags.insert("mountains".to_string());

        db.set_photo_tags(&path, new_tags.clone());
        assert_eq!(db.get_photo_tags(&path), new_tags);
    }

    #[test]
    fn test_all_tags() {
        let mut db = PhotoDatabase::new();
        let photo1 = create_test_photo("/test/photo1.jpg", None);
        let photo2 = create_test_photo("/test/photo2.jpg", None);

        db.add_photo(photo1);
        db.add_photo(photo2);

        db.add_photo_tag(&PathBuf::from("/test/photo1.jpg"), "landscape".to_string());
        db.add_photo_tag(&PathBuf::from("/test/photo1.jpg"), "sunset".to_string());
        db.add_photo_tag(&PathBuf::from("/test/photo2.jpg"), "portrait".to_string());
        db.add_photo_tag(&PathBuf::from("/test/photo2.jpg"), "landscape".to_string());

        let all_tags = db.all_tags();
        assert_eq!(all_tags.len(), 3);
        assert!(all_tags.contains(&"landscape".to_string()));
        assert!(all_tags.contains(&"sunset".to_string()));
        assert!(all_tags.contains(&"portrait".to_string()));

        // Test sorting
        assert_eq!(all_tags, vec!["landscape", "portrait", "sunset"]);
    }

    #[test]
    fn test_query_cache_invalidation() {
        let mut db = PhotoDatabase::new();
        let photo = create_test_photo("/test/photo1.jpg", None);
        let path = photo.path.clone();

        db.add_photo(photo);

        // Query to populate cache
        let query = PhotoQuery::default();
        db.query_photos(&query);
        assert!(!db.query_cache.is_empty());

        // Adding rating should invalidate cache
        db.set_photo_rating(&path, PhotoRating::Yes);
        assert!(db.query_cache.is_empty());

        // Query again to populate cache
        db.query_photos(&query);
        assert!(!db.query_cache.is_empty());

        // Adding tag should invalidate cache
        db.add_photo_tag(&path, "test".to_string());
        assert!(db.query_cache.is_empty());
    }

    #[test]
    fn test_query_grouped_by_rating() {
        let mut db = PhotoDatabase::new();
        let photo1 = create_test_photo("/test/photo1.jpg", None);
        let photo2 = create_test_photo("/test/photo2.jpg", None);
        let photo3 = create_test_photo("/test/photo3.jpg", None);

        db.add_photo(photo1);
        db.add_photo(photo2);
        db.add_photo(photo3);

        db.set_photo_rating(&PathBuf::from("/test/photo1.jpg"), PhotoRating::Yes);
        db.set_photo_rating(&PathBuf::from("/test/photo2.jpg"), PhotoRating::Yes);
        db.set_photo_rating(&PathBuf::from("/test/photo3.jpg"), PhotoRating::Maybe);

        let query = PhotoQuery {
            ratings: None,
            tags: None,
            grouping: PhotoGrouping::Rating,
        };

        let result = db.query_photos(&query);

        assert_eq!(result.groups.len(), 2); // Yes, Maybe (only groups with photos)

        let yes_group = result.groups.get("Yes").unwrap();
        assert_eq!(yes_group.len(), 2);
        assert!(yes_group.contains_key(&PathBuf::from("/test/photo1.jpg")));
        assert!(yes_group.contains_key(&PathBuf::from("/test/photo2.jpg")));

        let maybe_group = result.groups.get("Maybe").unwrap();
        assert_eq!(maybe_group.len(), 1);
        assert!(maybe_group.contains_key(&PathBuf::from("/test/photo3.jpg")));
    }

    #[test]
    fn test_query_grouped_by_date() {
        let mut db = PhotoDatabase::new();
        let dt1 = DateTime::parse_from_rfc3339("2023-01-01T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let dt2 = DateTime::parse_from_rfc3339("2023-01-02T15:30:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let photo1 = create_test_photo("/test/photo1.jpg", Some(dt1));
        let photo2 = create_test_photo("/test/photo2.jpg", Some(dt1));
        let photo3 = create_test_photo("/test/photo3.jpg", Some(dt2));
        let photo4 = create_test_photo("/test/photo4.jpg", None);

        db.add_photo(photo1);
        db.add_photo(photo2);
        db.add_photo(photo3);
        db.add_photo(photo4);

        let query = PhotoQuery {
            ratings: None,
            tags: None,
            grouping: PhotoGrouping::Date,
        };

        let result = db.query_photos(&query);

        assert!(result.groups.len() >= 2); // At least 2023-01-01, 2023-01-02, possibly "Unknown Date"

        let jan1_group = result.groups.get("2023-01-01").unwrap();
        assert_eq!(jan1_group.len(), 2);
        assert!(jan1_group.contains_key(&PathBuf::from("/test/photo1.jpg")));
        assert!(jan1_group.contains_key(&PathBuf::from("/test/photo2.jpg")));

        let jan2_group = result.groups.get("2023-01-02").unwrap();
        assert_eq!(jan2_group.len(), 1);
        assert!(jan2_group.contains_key(&PathBuf::from("/test/photo3.jpg")));
    }

    #[test]
    fn test_bidirectional_hash_map() {
        let mut map: BidirectionalHashMap<String, i32> = BidirectionalHashMap::new();

        map.insert("one".to_string(), 1);
        map.insert("two".to_string(), 2);

        assert_eq!(map.get_left(&"one".to_string()), Some(&1));
        assert_eq!(map.get_right(&1), Some(&"one".to_string()));

        assert_eq!(map.remove_left(&"one".to_string()), Some(1));
        assert_eq!(map.get_left(&"one".to_string()), None);
        assert_eq!(map.get_right(&1), None);

        assert_eq!(map.remove_right(&2), Some("two".to_string()));
        assert_eq!(map.get_left(&"two".to_string()), None);
        assert_eq!(map.get_right(&2), None);
    }

    #[test]
    fn test_query_cache() {
        let mut db = PhotoDatabase::new();
        let photo = create_test_photo("/test/photo1.jpg", None);

        db.add_photo(photo);

        let query = PhotoQuery::default();

        // First query should populate cache
        let result1 = db.query_photos(&query);
        assert_eq!(db.query_cache.len(), 1);

        // Second query should use cache
        let result2 = db.query_photos(&query);
        assert_eq!(result1, result2);

        // Manual cache invalidation
        db.invalidate_query_cache();
        assert_eq!(db.query_cache.len(), 0);
    }

    #[test]
    fn test_photo_after_within_group() {
        let mut db = PhotoDatabase::new();
        let dt1 = DateTime::parse_from_rfc3339("2023-01-01T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let dt2 = DateTime::parse_from_rfc3339("2023-01-01T13:00:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let photo1 = create_test_photo("/test/photo1.jpg", Some(dt1));
        let photo2 = create_test_photo("/test/photo2.jpg", Some(dt2));

        db.add_photo(photo1.clone());
        db.add_photo(photo2.clone());

        let query = PhotoQuery {
            ratings: None,
            tags: None,
            grouping: PhotoGrouping::Date,
        };

        let result = db.query_photos(&query);

        // Photos are sorted newest first, so photo2 (13:00) comes before photo1 (12:00)
        // Test moving to next photo within the same group (from photo2 to photo1)
        let next = result.photo_after(&photo2);
        assert!(next.is_some());
        assert_eq!(next.unwrap().path, photo1.path);
    }

    #[test]
    fn test_photo_after_across_groups() {
        let mut db = PhotoDatabase::new();
        let dt1 = DateTime::parse_from_rfc3339("2023-01-01T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let dt2 = DateTime::parse_from_rfc3339("2023-01-02T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let photo1 = create_test_photo("/test/photo1.jpg", Some(dt1));
        let photo2 = create_test_photo("/test/photo2.jpg", Some(dt2));

        db.add_photo(photo1.clone());
        db.add_photo(photo2.clone());

        let query = PhotoQuery {
            ratings: None,
            tags: None,
            grouping: PhotoGrouping::Date,
        };

        let result = db.query_photos(&query);

        // Test moving to first photo of next group
        let next = result.photo_after(&photo2);
        assert!(next.is_some());
        assert_eq!(next.unwrap().path, photo1.path);
    }

    #[test]
    fn test_photo_after_at_end() {
        let mut db = PhotoDatabase::new();
        let dt1 = DateTime::parse_from_rfc3339("2023-01-01T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let photo1 = create_test_photo("/test/photo1.jpg", Some(dt1));

        db.add_photo(photo1.clone());

        let query = PhotoQuery {
            ratings: None,
            tags: None,
            grouping: PhotoGrouping::Date,
        };

        let result = db.query_photos(&query);

        // Test that we get None when at the last photo
        let next = result.photo_after(&photo1);
        assert!(next.is_none());
    }

    #[test]
    fn test_photo_before_within_group() {
        let mut db = PhotoDatabase::new();
        let dt1 = DateTime::parse_from_rfc3339("2023-01-01T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let dt2 = DateTime::parse_from_rfc3339("2023-01-01T13:00:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let photo1 = create_test_photo("/test/photo1.jpg", Some(dt1));
        let photo2 = create_test_photo("/test/photo2.jpg", Some(dt2));

        db.add_photo(photo1.clone());
        db.add_photo(photo2.clone());

        let query = PhotoQuery {
            ratings: None,
            tags: None,
            grouping: PhotoGrouping::Date,
        };

        let result = db.query_photos(&query);

        // Photos are sorted newest first, so photo2 (13:00) comes before photo1 (12:00)
        // Test moving to previous photo within the same group (from photo1 to photo2)
        let prev = result.photo_before(&photo1);
        assert!(prev.is_some());
        assert_eq!(prev.unwrap().path, photo2.path);
    }

    #[test]
    fn test_photo_before_across_groups() {
        let mut db = PhotoDatabase::new();
        let dt1 = DateTime::parse_from_rfc3339("2023-01-01T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let dt2 = DateTime::parse_from_rfc3339("2023-01-02T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let photo1 = create_test_photo("/test/photo1.jpg", Some(dt1));
        let photo2 = create_test_photo("/test/photo2.jpg", Some(dt2));

        db.add_photo(photo1.clone());
        db.add_photo(photo2.clone());

        let query = PhotoQuery {
            ratings: None,
            tags: None,
            grouping: PhotoGrouping::Date,
        };

        let result = db.query_photos(&query);

        // Test moving to last photo of previous group
        let prev = result.photo_before(&photo1);
        assert!(prev.is_some());
        assert_eq!(prev.unwrap().path, photo2.path);
    }

    #[test]
    fn test_photo_before_at_beginning() {
        let mut db = PhotoDatabase::new();
        let dt1 = DateTime::parse_from_rfc3339("2023-01-01T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let photo1 = create_test_photo("/test/photo1.jpg", Some(dt1));

        db.add_photo(photo1.clone());

        let query = PhotoQuery {
            ratings: None,
            tags: None,
            grouping: PhotoGrouping::Date,
        };

        let result = db.query_photos(&query);

        // Test that we get None when at the first photo
        let prev = result.photo_before(&photo1);
        assert!(prev.is_none());
    }

    #[test]
    fn test_photo_query_result_iterator_forward() {
        let mut db = PhotoDatabase::new();
        let dt1 = DateTime::parse_from_rfc3339("2023-01-01T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let dt2 = DateTime::parse_from_rfc3339("2023-01-02T12:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let dt3 = DateTime::parse_from_rfc3339("2023-01-02T13:00:00Z")
            .unwrap()
            .with_timezone(&Utc);

        let photo1 = create_test_photo("/test/photo1.jpg", Some(dt1));
        let photo2 = create_test_photo("/test/photo2.jpg", Some(dt2));
        let photo3 = create_test_photo("/test/photo3.jpg", Some(dt3));

        db.add_photo(photo1.clone());
        db.add_photo(photo2.clone());
        db.add_photo(photo3.clone());

        let query = PhotoQuery {
            ratings: None,
            tags: None,
            grouping: PhotoGrouping::Date,
        };

        let result = db.query_photos(&query);
        let items: Vec<_> = result.clone().into_iter().collect();

        // We should have 3 items (photos are sorted newest first in query)
        assert_eq!(items.len(), 3);

        // Check that we iterate through all photos
        let paths: Vec<PathBuf> = items.iter().map(|(_, photo)| photo.path.clone()).collect();
        assert!(paths.contains(&PathBuf::from("/test/photo1.jpg")));
        assert!(paths.contains(&PathBuf::from("/test/photo2.jpg")));
        assert!(paths.contains(&PathBuf::from("/test/photo3.jpg")));
    }
}

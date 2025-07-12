use std::{collections::HashMap, hash::Hash, path::PathBuf};

use indexmap::IndexMap;
use once_cell::sync::Lazy;
use parking_lot::Mutex;
use savefile_derive::Savefile;

use crate::photo::Photo;

enum PhotoDatabaseError {}

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
}

impl PhotoDatabase {
    pub fn new() -> Self {
        Self {
            photos: Vec::new(),
            path_map: BidirectionalHashMap::new(),
        }
    }

    pub fn add_photo(&mut self, photo: Photo) {
        self.path_map.insert(self.photos.len(), photo.path.clone());
        self.photos.push(photo);
    }

    pub fn get_photo(&self, path: &PathBuf) -> Option<&Photo> {
        self.path_map
            .get_right(path)
            .map(|index| &self.photos[*index])
    }

    pub fn get_photo_mut(&mut self, path: &PathBuf) -> Option<&mut Photo> {
        // self.photos.get_mut(path)
        None
    }

    pub fn get_photo_by_index(&self, index: usize) -> Option<&Photo> {
        // self.photos.get_index(index).map(|(_, photo)| photo)
        None
    }

    pub fn get_photo_by_index_mut(&mut self, index: usize) -> Option<&mut Photo> {
        // self.photos.get_index_mut(index).map(|(_, photo)| photo)
        None
    }

    pub fn remove_photo(&mut self, path: &PathBuf) {
        if let Some(index) = self.path_map.remove_right(path) {
            self.photos.swap_remove(index);
        }
    }
}

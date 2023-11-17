use std::{
    marker::PhantomData,
    sync::{Arc, Mutex, MutexGuard},
};

use crate::{image_cache::ImageCache, event_bus::{EventBus, EventBusId}, photo_manager::PhotoManager};

macro_rules! singleton {
    ($name: ident, $type:ty, $init:expr) => {

        static $name: once_cell::sync::Lazy<Singleton<$type>> = once_cell::sync::Lazy::new(|| Singleton(Arc::new(Mutex::new($init))));
        
        impl SingletonFor<$type> for Dependency<$type> {
            fn get() -> Singleton<$type> {
                ($name).clone()
            }
        }
    };
}

macro_rules! dependency {
    ($type:ty, $init: expr) => {

        impl DependencyFor<$type> for Dependency<$type> {
            fn get() -> $type {
                $init
            }
        }
    };
}

pub struct Singleton<T>(Arc<Mutex<T>>);


impl<T> Clone for Singleton<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> Singleton<T> {
    pub fn with_lock<R>(&self, op: impl FnOnce(&MutexGuard<'_, T>) -> R) -> R {
        op(&self.0.lock().expect("Failed to lock singleton"))
    }

    pub fn with_lock_mut<R>(&self, op: impl FnOnce(&mut MutexGuard<'_, T>) -> R) -> R {
        op(&mut self.0.lock().expect("Failed to lock singleton"))
    }
}

impl <E: Clone> Singleton<EventBus<E>> {

    pub fn emit(&self, event: E) {
        self.with_lock_mut(|event_bus| event_bus.emit(event));
    }

    pub fn listen(&self, id: EventBusId, listener: impl Fn(E) -> () + 'static) {
        self.with_lock_mut(|event_bus| event_bus.listen(id, Box::new(listener)));
    }
}

pub trait DependencyFor<T> {
    fn get() -> T;
}

pub trait SingletonFor<T> {
    fn get() -> Singleton<T>;
}


pub struct Dependency<T>(PhantomData<T>);

singleton!(
    IMAGE_CACHE_INSTANCE,
    ImageCache,
    ImageCache::new()
);

// singleton!(
//     GALLERY_IMAGE_EVENT_BUS_INSTANCE,
//     EventBus<GalleryImageEvent>,
//     EventBus::new()
// );

singleton!(
    PHOTO_MANAGER_INSTANCE,
    PhotoManager,
    PhotoManager::new()
);
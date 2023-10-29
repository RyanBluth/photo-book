use std::{
    marker::PhantomData,
    sync::{Arc, Mutex, MutexGuard},
    rc::Rc,
};

use crate::{image_cache::ImageCache, gallery_service::ThumbnailService, event_bus::{EventBus, GalleryImageEvent, EventBusId}};

macro_rules! singleton {
    ($name: ident, $type:ty, $init:expr) => {
        const $name: once_cell::unsync::Lazy<Singleton<$type>> = once_cell::unsync::Lazy::new(|| Singleton(Rc::new(Mutex::new($init))));

        impl SingletonFor<$type> for Dependency<$type> {
            fn get() -> Singleton<$type> {
                ($name).clone()
            }
        }
    };
}

macro_rules! send_singleton {
    ($name: ident, $type:ty, $init:expr) => {
        static $name: once_cell::sync::Lazy<SendableSingleton<$type>> = once_cell::sync::Lazy::new(|| SendableSingleton(Arc::new(Mutex::new($init))));

        impl SendableSingletonFor<$type> for Dependency<$type> {
            fn get() -> SendableSingleton<$type> {
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

pub struct Singleton<T>(Rc<Mutex<T>>);


impl<T> Clone for Singleton<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

pub struct SendableSingleton<T>(Arc<Mutex<T>>);

impl<T> Clone for SendableSingleton<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}


impl<T> Singleton<T> {
    pub fn with_lock<R>(&self, op: impl FnOnce(&MutexGuard<'_, T>) -> R) -> anyhow::Result<R> {
        match self.0.lock() {
            Ok(guard) => Ok(op(&guard)),
            Err(e) => Err(anyhow::anyhow!("Failed to lock singleton: {}", e)),
        }
    }

    pub fn with_lock_mut<R>(&self, op: impl FnOnce(&mut MutexGuard<'_, T>) -> R) -> anyhow::Result<R> {
        match self.0.lock() {
            Ok(mut guard) => Ok(op(&mut guard)),
            Err(e) => Err(anyhow::anyhow!("Failed to lock singleton: {}", e)),
        }
    }
}

impl<T> SendableSingleton<T> {
    pub fn with_lock<R>(&self, op: impl FnOnce(&MutexGuard<'_, T>) -> R) -> anyhow::Result<R> {
        match self.0.lock() {
            Ok(guard) => Ok(op(&guard)),
            Err(e) => Err(anyhow::anyhow!("Failed to lock singleton: {}", e)),
        }
    }

    pub fn with_lock_mut<R>(&self, op: impl FnOnce(&mut MutexGuard<'_, T>) -> R) -> anyhow::Result<R> {
        match self.0.lock() {
            Ok(mut guard) => Ok(op(&mut guard)),
            Err(e) => Err(anyhow::anyhow!("Failed to lock singleton: {}", e)),
        }
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

pub trait SendableSingletonFor<T> {
    fn get() -> SendableSingleton<T>;
}

pub struct Dependency<T>(PhantomData<T>);

send_singleton!(
    IMAGE_CACHE_INSTANCE,
    ImageCache,
    ImageCache::new()
);

singleton!(
    GALLERY_IMAGE_EVENT_BUS_INSTANCE,
    EventBus<GalleryImageEvent>,
    EventBus::new()
);

dependency!(
    ThumbnailService,
    ThumbnailService::new()
);
use std::{
    marker::PhantomData,
    sync::{Arc, Mutex, MutexGuard},
};

use crate::thumbnail_cache::ThumbnailCache;

use once_cell::sync::Lazy;

macro_rules! singleton {
    ($name: ident, $type:ty, $init:expr) => {
        static $name: Lazy<Singleton<$type>> = Lazy::new(|| Singleton(Arc::new(Mutex::new($init))));

        impl UsingSingletonMut<$type> for Dependency<$type> {
            fn using_singleton_mut<R>(op: impl FnOnce(&mut MutexGuard<'_, $type>) -> R) -> R {
                Self::get().using_mut(op)
            }
        }
    
        impl UsingSingleton<$type> for Dependency<$type> {
            fn using_singleton<R>(op: impl FnOnce(&MutexGuard<'_, $type>) -> R) -> R {
                Self::get().using(op)
            }
        }

        impl SingletonFor<$type> for Dependency<$type> {
            fn get() -> Singleton<$type> {
                (*$name).clone()
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
    fn using<R>(&self, op: impl FnOnce(&MutexGuard<'_, T>) -> R) -> R {
        op(&mut self.0.lock().unwrap())
    }

    fn using_mut<R>(&self, op: impl FnOnce(&mut MutexGuard<'_, T>) -> R) -> R {
        op(&mut self.0.lock().unwrap())
    }
}

pub trait DependencyFor<T> {
    fn get() -> T;
}

pub trait SingletonFor<T> {
    fn get() -> Singleton<T>;
}

pub trait UsingSingletonMut<T> {
    fn using_singleton_mut<R>(op: impl FnOnce(&mut MutexGuard<'_, T>) -> R) -> R;
}

pub trait UsingSingleton<T> {
    fn using_singleton<R>(op: impl FnOnce(&MutexGuard<'_, T>) -> R) -> R;
}

pub struct Dependency<T>(PhantomData<T>);

singleton!(
    THUMBNAIL_CACHE_INSTANCE,
    ThumbnailCache,
    ThumbnailCache::new()
);
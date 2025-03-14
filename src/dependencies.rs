use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::{marker::PhantomData, sync::Arc};

use crate::{
    auto_persisting::AutoPersisting, autosave_manager::AutoSaveManager, config::Config,
    cursor_manager::CursorManager, debug::DebugSettings, export::Exporter,
    font_manager::FontManager, modal::manager::ModalManager, photo_manager::PhotoManager,
    project_settings::ProjectSettingsManager, session::Session,
};

macro_rules! singleton {
    ($name: ident, $type:ty, $init:expr_2021) => {
        static $name: once_cell::sync::Lazy<Singleton<$type>> =
            once_cell::sync::Lazy::new(|| Singleton(Arc::new(RwLock::new($init))));

        impl SingletonFor<$type> for Dependency<$type> {
            fn get() -> Singleton<$type> {
                ($name).clone()
            }
        }
    };
}

macro_rules! dependency {
    ($type:ty, $init: expr_2021) => {
        impl DependencyFor<$type> for Dependency<$type> {
            fn get() -> $type {
                $init
            }
        }
    };
}

#[derive(Debug)]
pub struct Singleton<T>(Arc<RwLock<T>>);

impl<T> Singleton<T> {
    pub fn new(value: T) -> Self {
        Self(Arc::new(RwLock::new(value)))
    }
}

impl<T> Clone for Singleton<T> {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl<T> Singleton<T> {
    pub fn with_lock<R>(&self, op: impl FnOnce(&RwLockReadGuard<'_, T>) -> R) -> R {
        op(&self.0.read())
    }

    pub fn with_lock_mut<R>(&self, op: impl FnOnce(&mut RwLockWriteGuard<'_, T>) -> R) -> R {
        op(&mut self.0.write())
    }
}

pub trait DependencyFor<T> {
    fn get() -> T;
}

pub trait SingletonFor<T> {
    fn get() -> Singleton<T>;
}

pub struct Dependency<T>(PhantomData<T>);

singleton!(PHOTO_MANAGER_INSTANCE, PhotoManager, PhotoManager::new());

singleton!(CURSOR_MANAGER, CursorManager, CursorManager::new());

singleton!(FONT_MANAGER, FontManager, FontManager::new());

singleton!(EXPORTER, Exporter, Exporter::new());

singleton!(CONFIG, AutoPersisting<Config>, AutoPersisting::new());

singleton!(MODAL_MANAGER, ModalManager, ModalManager::new());

singleton!(
    PROJECT_SETTINGS_MANAGER,
    ProjectSettingsManager,
    ProjectSettingsManager::new()
);

singleton!(AUTOSAVE_MANAGER, AutoSaveManager, AutoSaveManager::new());

singleton!(SESSION, Session, Session::new());

singleton!(DEBUG_SETTINGS, DebugSettings, DebugSettings::default());

use parking_lot::{RwLock, RwLockReadGuard, RwLockWriteGuard};
use std::{marker::PhantomData, sync::Arc};

use crate::{
    auto_persisting::AutoPersisting, autosave_manager::AutoSaveManager, config::Config,
    cursor_manager::CursorManager, debug::DebugSettings,
    deferred_work_manager::DeferredWorkManager, export::Exporter, font_manager::FontManager,
    modal::manager::ModalManager, photo_manager::PhotoManager,
    project_settings::ProjectSettingsManager, selection_manager::SelectionManager,
    session::Session, sizing_manager::SizingManager, string_log::StringLog,
};

macro_rules! singleton {
    ($name: ident, $type:ty, $init:expr_2021) => {
        static $name: once_cell::sync::Lazy<Singleton<$type>> =
            once_cell::sync::Lazy::new(|| Singleton::new_with_name($init, stringify!($name)));

        impl SingletonFor<$type> for Dependency<$type> {
            fn get() -> Singleton<$type> {
                ($name).clone()
            }
        }
    };
}

#[allow(unused_macros)]
macro_rules! dependency {
    ($type:ty, $init: expr_2021) => {
        impl DependencyFor<$type> for Dependency<$type> {
            fn get() -> $type {
                $init
            }
        }
    };
}

#[macro_export]
macro_rules! dep {
    ($type:ty, |$dep:pat_param| $body:expr $(,)?) => {{
        type Dependency = $crate::dependencies::Dependency<$type>;
        <Dependency as $crate::dependencies::SingletonFor<$type>>::get().with_lock(|$dep| $body)
    }};
}

#[macro_export]
macro_rules! dep_mut {
    ($type:ty, |$dep:pat_param| $body:expr $(,)?) => {{
        type Dependency = $crate::dependencies::Dependency<$type>;
        <Dependency as $crate::dependencies::SingletonFor<$type>>::get().with_lock_mut(|$dep| $body)
    }};
}

#[derive(Debug)]
pub struct Singleton<T> {
    lock: Arc<RwLock<T>>,
    name: &'static str,
}

impl<T> Singleton<T> {
    #[allow(dead_code)]
    pub fn new(value: T) -> Self {
        Self {
            lock: Arc::new(RwLock::new(value)),
            name: "Anonymous",
        }
    }

    pub fn new_with_name(value: T, name: &'static str) -> Self {
        Self {
            lock: Arc::new(RwLock::new(value)),
            name,
        }
    }
}

impl<T> Clone for Singleton<T> {
    fn clone(&self) -> Self {
        Self {
            lock: self.lock.clone(),
            name: self.name,
        }
    }
}

#[cfg(not(feature = "debug_dependency_locks"))]
impl<T> Singleton<T> {
    pub fn with_lock<R>(&self, op: impl FnOnce(&RwLockReadGuard<'_, T>) -> R) -> R {
        op(&self.lock.read())
    }

    pub fn with_lock_mut<R>(&self, op: impl FnOnce(&mut RwLockWriteGuard<'_, T>) -> R) -> R {
        op(&mut self.lock.write())
    }
}

#[allow(dead_code)]
pub trait DependencyFor<T> {
    fn get() -> T;
}

pub trait SingletonFor<T> {
    fn get() -> Singleton<T>;
}

pub struct Dependency<T>(PhantomData<T>);

singleton!(PHOTO_MANAGER_INSTANCE, PhotoManager, PhotoManager::new());

singleton!(SELECTION_MANAGER, SelectionManager, SelectionManager::new());

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

singleton!(STRING_LOG, StringLog, StringLog::new());

singleton!(
    DEFERRED_WORK_MANAGER,
    DeferredWorkManager,
    DeferredWorkManager::new()
);

singleton!(SIZING_MANAGER, SizingManager, SizingManager::new());

#[allow(unused_imports)]
use backtrace::Backtrace;
#[allow(unused_imports)]
use once_cell::sync::Lazy;
#[allow(unused_imports)]
use parking_lot::Mutex;
#[allow(unused_imports)]
use std::collections::HashMap;

#[cfg(feature = "debug_dependency_locks")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LockType {
    Read,
    Write,
}

#[cfg(feature = "debug_dependency_locks")]
#[derive(Debug, Clone)]
struct LockInfo {
    lock_type: LockType,
    backtrace: Backtrace,
    singleton_name: &'static str,
}

#[cfg(feature = "debug_dependency_locks")]
static ACTIVE_LOCKS: Lazy<Mutex<HashMap<(std::thread::ThreadId, usize), LockInfo>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

#[cfg(feature = "debug_dependency_locks")]
pub struct LockGuard<'a, T> {
    pub singleton: &'a Singleton<T>,
}

#[cfg(feature = "debug_dependency_locks")]
impl<'a, T> LockGuard<'a, T> {
    fn new(singleton: &'a Singleton<T>, lock_type: LockType) -> Self {
        singleton.register_lock(lock_type);
        Self { singleton }
    }
}

#[cfg(feature = "debug_dependency_locks")]
impl<'a, T> Drop for LockGuard<'a, T> {
    fn drop(&mut self) {
        self.singleton.unregister_lock();
    }
}

#[cfg(feature = "debug_dependency_locks")]
impl<T> Singleton<T> {
    fn lock_id(&self) -> usize {
        Arc::as_ptr(&self.lock) as usize
    }

    fn print_filtered_backtrace(bt: &Backtrace) {
        let frames: Vec<_> = bt.frames().iter().collect();
        for frame in frames {
            for symbol in frame.symbols() {
                if let Some(filename) = symbol.filename() {
                    let path_str = filename.to_string_lossy();
                    // Only print frames from our source code
                    if path_str.contains("photobook-rs") {
                        let name = symbol
                            .name()
                            .map(|n| n.to_string())
                            .unwrap_or_else(|| "<unknown>".to_string());
                        let line = symbol.lineno().unwrap_or(0);
                        eprintln!("      at {}:{}", path_str, line);
                        eprintln!("         {}", name);
                    }
                }
            }
        }
    }

    fn register_lock(&self, lock_type: LockType) {
        let thread_id = std::thread::current().id();
        let lock_id = self.lock_id();
        let key = (thread_id, lock_id);

        let mut locks = ACTIVE_LOCKS.lock();

        // Only print if there are existing locks
        if !locks.is_empty() {
            eprintln!("\n=== ACQUIRING LOCK ===");
            eprintln!("Singleton: {}", self.name);
            eprintln!("Lock type: {:?}", lock_type);
            eprintln!("Thread: {:?}", thread_id);

            let current_backtrace = Backtrace::new();
            eprintln!("\nBacktrace (project code only):");
            Self::print_filtered_backtrace(&current_backtrace);

            eprintln!("\nCurrent active locks ({} total):", locks.len());
            for ((tid, lid), info) in locks.iter() {
                eprintln!(
                    "  - {} ({:?}) on thread {:?} (lock addr: 0x{:x})",
                    info.singleton_name, info.lock_type, tid, lid
                );
                eprintln!("    Acquired at:");
                Self::print_filtered_backtrace(&info.backtrace);
            }
            eprintln!("======================\n");
        }

        locks.insert(
            key,
            LockInfo {
                lock_type,
                backtrace: Backtrace::new(),
                singleton_name: self.name,
            },
        );
    }

    pub(super) fn unregister_lock(&self) {
        let thread_id = std::thread::current().id();
        let lock_id = self.lock_id();
        let key = (thread_id, lock_id);

        let mut locks = ACTIVE_LOCKS.lock();
        locks.remove(&key);
    }

    pub fn with_lock<R>(&self, op: impl FnOnce(&RwLockReadGuard<'_, T>) -> R) -> R {
        let _guard = LockGuard::new(self, LockType::Read);
        op(&self.lock.read())
    }

    pub fn with_lock_mut<R>(&self, op: impl FnOnce(&mut RwLockWriteGuard<'_, T>) -> R) -> R {
        let _guard = LockGuard::new(self, LockType::Write);
        op(&mut self.lock.write())
    }
}

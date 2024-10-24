#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use std::sync::Arc;

use autosave_manager::AutoSaveManager;
use cursor_manager::CursorManager;
use dependencies::{Dependency, DependencyFor, Singleton, SingletonFor};
use eframe::egui::{self, ViewportBuilder, Widget};

use font_manager::FontManager;

use dirs::Dirs;
use log::info;
use modal::manager::ModalManager;
use photo_manager::PhotoManager;
use scene::SceneManager;
use tokio::runtime;

use flexi_logger::{Logger, WriteMode};
use string_log::{ArcStringLog, StringLog};

mod assets;
mod auto_persisting;
mod autosave_manager;
mod config;
mod cursor_manager;
mod dependencies;
mod dirs;
mod error_sink;
mod export;
mod font_manager;
mod history;
mod id;
mod modal;
mod model;
mod photo;
mod photo_manager;
mod project;
mod project_settings;
mod scene;
mod string_log;
mod template;
mod theme;
mod utils;
mod widget;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let log: Arc<StringLog> = Arc::new(StringLog::new());

    rayon::ThreadPoolBuilder::new()
        .num_threads(4)
        .build_global()
        .unwrap();

    let rt = runtime::Builder::new_multi_thread()
        .enable_all()
        .max_blocking_threads(16)
        .build()
        .unwrap();

    Dirs::initialize_dirs();

    // Enter the runtime so that `tokio::spawn` is available immediately.
    let _enter = rt.enter();

    let _logger = Logger::try_with_str("info, my::critical::module=trace")
        .unwrap()
        .log_to_writer(Box::new(ArcStringLog::new(Arc::clone(&log))))
        .write_mode(WriteMode::Direct)
        .start()?;

    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_maximize_button(true)
            .with_inner_size((3000.0, 2000.0)),
        ..Default::default()
    };

    let app_log = Arc::clone(&log);

    eframe::run_native(
        "Show an image with eframe/egui",
        options,
        Box::new(|_cc| Ok(Box::<PhotoBookApp>::new(PhotoBookApp::new(app_log)))),
    )
    .map_err(|e| anyhow::anyhow!("Error running native app: {}", e))
}

#[derive(Debug, Clone, PartialEq)]
enum PrimaryComponentKind {
    Gallery = 0,
    Viewer = 1,
    Canvas = 2,
    PhotoInfo = 3,
    CanvasInfo = 4,
}

struct PhotoBookApp {
    log: Arc<StringLog>,
    photo_manager: Singleton<PhotoManager>,
    loaded_fonts: bool,
    scene_manager: SceneManager,
}

impl PhotoBookApp {
    fn new(log: Arc<StringLog>) -> Self {
        Self {
            photo_manager: Dependency::<PhotoManager>::get(),
            log,

            loaded_fonts: false,
            scene_manager: SceneManager::default(),
        }
    }
}

impl eframe::App for PhotoBookApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui_extras::install_image_loaders(ctx);

        ctx.input_mut(|input| {
            input.max_texture_side = usize::MAX; // TODO: What are the consequences of doing this?
        });

        if !self.loaded_fonts {
            self.loaded_fonts = true;
            // Just load all fonts at start up. Maybe there's a better time to do this?
            let font_manager: Singleton<FontManager> = Dependency::get();
            font_manager.with_lock_mut(|font_manager| {
                font_manager.load_fonts(ctx);
            });
        }

        Dependency::<CursorManager>::get().with_lock_mut(|cursor_manager| {
            cursor_manager.begin_frame(ctx);
        });

        egui::CentralPanel::default().show(ctx, |ui| {
            self.scene_manager.ui(ui);

            let modal_manager: Singleton<ModalManager> = Dependency::get();
            modal_manager.with_lock_mut(|modal_manager| {
                modal_manager.show_next(ui);
            });
        });

        Dependency::<CursorManager>::get().with_lock_mut(|cursor_manager| {
            cursor_manager.end_frame(ctx);
        });

        Dependency::<AutoSaveManager>::get().with_lock_mut(|auto_save_manager| {
            let _ = auto_save_manager.auto_save_if_needed(&self.scene_manager.root_scene);
        });
    }
}

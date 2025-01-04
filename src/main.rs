#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use std::sync::Arc;

use auto_persisting::AutoPersisting;
use autosave_manager::AutoSaveManager;
use config::Config;
use cursor_manager::CursorManager;
use dependencies::{Dependency, DependencyFor, Singleton, SingletonFor};
use eframe::{
    egui::{self, ViewportBuilder, Widget},
    // egui_wgpu::{WgpuConfiguration, WgpuSetup},
    //wgpu,
};

use font_manager::FontManager;

use dirs::Dirs;
use log::info;
use modal::manager::ModalManager;
use photo_manager::PhotoManager;
use project::v1::Project;
use scene::{organize_edit_scene::OrganizeEditScene, SceneManager};
use tokio::runtime;

use flexi_logger::{Logger, WriteMode};
use string_log::{ArcStringLog, StringLog};

mod assets;
mod auto_persisting;
mod autosave_manager;
mod config;
mod cursor_manager;
mod debug;
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
mod session;
mod string_log;
mod template;
mod theme;
mod utils;
mod widget;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let log: Arc<StringLog> = Arc::new(StringLog::new());

    let num_cores: i32 = num_cpus::get() as i32;

    let rt = runtime::Builder::new_multi_thread()
        .enable_all()
        .max_blocking_threads((num_cores - 2).max(1) as usize)
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
        hardware_acceleration: eframe::HardwareAcceleration::Required,
        // wgpu_options: WgpuConfiguration {
        //     wgpu_setup: WgpuSetup::CreateNew {
        //         supported_backends: wgpu::util::backend_bits_from_env()
        //             .unwrap_or(wgpu::Backends::PRIMARY | wgpu::Backends::GL),
        //         power_preference: wgpu::PowerPreference::HighPerformance,
        //         device_descriptor: Arc::new(|adapter| {
        //             let base_limits: wgpu::Limits =
        //                 if adapter.get_info().backend == wgpu::Backend::Gl {
        //                     wgpu::Limits::downlevel_webgl2_defaults()
        //                 } else {
        //                     wgpu::Limits::default()
        //                 };

        //             wgpu::DeviceDescriptor {
        //                 label: Some("egui wgpu device"),
        //                 required_features: wgpu::Features::default(),
        //                 required_limits: wgpu::Limits {
        //                     max_texture_dimension_2d: base_limits.max_texture_dimension_2d, // TODO: Can we look up the max size?
        //                     ..base_limits
        //                 },
        //                 memory_hints: wgpu::MemoryHints::default(),
        //             }
        //         }),
        //     },
        //     ..Default::default()
        // },
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
    loaded_initial_scene: bool,
}

impl PhotoBookApp {
    fn new(log: Arc<StringLog>) -> Self {
        Self {
            photo_manager: Dependency::<PhotoManager>::get(),
            log,
            loaded_fonts: false,
            scene_manager: SceneManager::default(),
            loaded_initial_scene: false,
        }
    }

    fn initialize_scene_manager() -> SceneManager {
        let config = Dependency::<AutoPersisting<Config>>::get();
        let last_project_path = config.with_lock_mut(|config| {
            config
                .read()
                .ok()
                .and_then(|config| config.last_project().cloned())
        });

        if let Some(scene) = Self::try_load_auto_save() {
            return SceneManager::new(scene);
        }

        if let Some(scene) = Self::try_load_last_project(&last_project_path) {
            return SceneManager::new(scene);
        }

        SceneManager::default()
    }

    fn try_load_auto_save() -> Option<OrganizeEditScene> {
        let auto_save_time = AutoSaveManager::get_auto_save_modification_time()?;
        let last_project_time = Self::get_last_project_time()?;

        if auto_save_time > last_project_time {
            AutoSaveManager::load_auto_save()
        } else {
            None
        }
    }

    fn try_load_last_project(
        project_path: &Option<std::path::PathBuf>,
    ) -> Option<OrganizeEditScene> {
        let path = project_path.as_ref()?;
        match Project::load(path) {
            Ok(scene) => Some(scene),
            Err(e) => {
                info!("Failed to load project: {:?}", e);
                None
            }
        }
    }

    fn get_last_project_time() -> Option<std::time::SystemTime> {
        let config = Dependency::<AutoPersisting<Config>>::get();
        let last_project_path = config.with_lock_mut(|config| {
            config
                .read()
                .ok()
                .and_then(|config| config.last_project().cloned())
        })?;

        std::fs::metadata(last_project_path).ok()?.modified().ok()
    }
}

impl eframe::App for PhotoBookApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.loaded_initial_scene {
            egui_extras::install_image_loaders(ctx);

            ctx.input_mut(|input| {
                input.max_texture_side = usize::MAX; // Allow maximum possible texture size
            });

            self.loaded_initial_scene = true;
            self.scene_manager = Self::initialize_scene_manager();
        }

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

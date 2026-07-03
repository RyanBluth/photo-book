#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU32, Ordering},
    },
    time::SystemTime,
};

use auto_persisting::AutoPersisting;
use autosave_manager::AutoSaveManager;
use config::Config;
use cursor_manager::CursorManager;
use eframe::{
    Frame,
    egui::{self, Context, Ui, ViewportBuilder},
};

use egui::Color32;
use font_manager::FontManager;

use dirs::Dirs;
use log::info;
use modal::manager::ModalManager;
use project::Project;
use scene::{SceneManager, organize_edit_scene::OrganizeEditScene};
use tokio::runtime;

use flexi_logger::{Logger, WriteMode};
use string_log::StringLogWriter;
use wgpu::Color;

use crate::deferred_work_manager::DeferredWorkManager;

mod assets;
mod auto_persisting;
mod autosave_manager;
mod config;
mod cursor_manager;
mod debug;
mod deferred_work_manager;
mod dependencies;
mod dirs;
mod error_sink;
mod export;
mod file_tree;
mod font_manager;
mod history;
mod id;
mod layout;
mod modal;
mod model;
#[cfg(target_os = "macos")]
mod native_mac_menu;
mod photo;
mod photo_database;
mod photo_manager;
mod project;
mod project_settings;
mod scene;
mod selection_manager;
mod session;
mod sizing_manager;
mod string_log;
mod template;
mod theme;
mod utils;
mod widget;
mod app_status;

static MAX_TEXTURE_SIZE: AtomicU32 = AtomicU32::new(0);

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let num_cores: i32 = num_cpus::get() as i32;

    let rt = runtime::Builder::new_multi_thread()
        .enable_all()
        .max_blocking_threads((num_cores - 2).max(1) as usize)
        .build()
        .unwrap();

    Dirs::initialize_dirs();

    // Enter the runtime so that `tokio::spawn` is available immediately.
    let _enter = rt.enter();

    // Start deadlock detection thread
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(std::time::Duration::from_secs(3));
            let deadlocks = parking_lot::deadlock::check_deadlock();
            if !deadlocks.is_empty() {
                eprintln!("\n{} deadlock(s) detected!", deadlocks.len());
                for (i, threads) in deadlocks.iter().enumerate() {
                    eprintln!("\nDeadlock #{}", i);
                    for thread in threads {
                        eprintln!("Thread ID: {:?}", thread.thread_id());
                        eprintln!("Backtrace:");
                        eprintln!("{:?}", thread.backtrace());
                        eprintln!();
                    }
                }
            }
        }
    });

    let _logger = Logger::try_with_str("info, my::critical::module=trace")
        .unwrap()
        .log_to_writer(Box::new(StringLogWriter))
        .write_mode(WriteMode::Direct)
        .start()?;

    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_maximize_button(true)
            .with_inner_size((3000.0, 2000.0)),
        hardware_acceleration: eframe::HardwareAcceleration::Required,
        renderer: eframe::Renderer::Wgpu,
        wgpu_options: eframe::egui_wgpu::WgpuConfiguration {
            wgpu_setup: eframe::egui_wgpu::WgpuSetup::CreateNew(
                eframe::egui_wgpu::WgpuSetupCreateNew {
                    power_preference: wgpu::PowerPreference::HighPerformance,
                    device_descriptor: Arc::new(|adapter| {
                        let base_limits: wgpu::Limits =
                            if adapter.get_info().backend == wgpu::Backend::Gl {
                                wgpu::Limits::downlevel_webgl2_defaults()
                            } else {
                                wgpu::Limits::default()
                            };

                        let adapter_limits = adapter.limits();
                        let safe_texture_limit = adapter_limits.max_texture_dimension_2d.min(32768);

                        MAX_TEXTURE_SIZE.store(safe_texture_limit, Ordering::Relaxed);

                        info!("GPU adapter: {}", adapter.get_info().name);
                        info!("GPU backend: {:?}", adapter.get_info().backend);
                        info!(
                            "Max texture dimension (hardware): {}",
                            adapter_limits.max_texture_dimension_2d
                        );
                        info!(
                            "Max texture dimension (application): {}",
                            safe_texture_limit
                        );

                        wgpu::DeviceDescriptor {
                            label: Some("egui wgpu device"),
                            required_features: wgpu::Features::default(),
                            required_limits: wgpu::Limits {
                                max_texture_dimension_2d: safe_texture_limit,
                                ..base_limits
                            },
                            memory_hints: wgpu::MemoryHints::default(),
                            trace: wgpu::Trace::Off,
                            experimental_features: wgpu::ExperimentalFeatures::default(),
                        }
                    }),
                    ..eframe::egui_wgpu::WgpuSetupCreateNew::without_display_handle()
                },
            ),
            ..Default::default()
        },
        ..Default::default()
    };

    eframe::run_native(
        "Photobook",
        options,
        Box::new(|_cc| {
            #[cfg(target_os = "linux")]
            _cc.egui_ctx.enable_accesskit();

            //re_ui::apply_style_and_install_loaders(&cc.egui_ctx);
            let mut app = PhotoBookApp::new();
            #[cfg(target_os = "macos")]
            {
                app.menu_handler = native_mac_menu::install();
            }
            Ok(Box::new(app))
        }),
    )
    .map_err(|e| anyhow::anyhow!("Error running native app: {}", e))
}

#[derive(Debug, Clone, PartialEq)]
enum _PrimaryComponentKind {
    Gallery = 0,
    Viewer = 1,
    Canvas = 2,
    PhotoInfo = 3,
    CanvasInfo = 4,
}

struct PhotoBookApp {
    loaded_fonts: bool,
    scene_manager: SceneManager,
    loaded_initial_scene: bool,
    #[cfg(target_os = "macos")]
    menu_handler: Option<objc2::rc::Retained<native_mac_menu::NativeMenuHandler>>,
}

impl PhotoBookApp {
    fn new() -> Self {
        Self {
            loaded_fonts: false,
            scene_manager: SceneManager::default(),
            loaded_initial_scene: false,
            #[cfg(target_os = "macos")]
            menu_handler: None,
        }
    }

    fn initialize_scene_manager() -> SceneManager {
        let last_project_path = dep_mut!(AutoPersisting<Config>, |config| {
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
        let last_project_time = Self::get_last_project_time();

        match last_project_time {
            Some(time) => {
                if auto_save_time > time {
                    AutoSaveManager::load_auto_save()
                } else {
                    None
                }
            }
            None => AutoSaveManager::load_auto_save(),
        }
    }

    fn try_load_last_project(project_path: &Option<PathBuf>) -> Option<OrganizeEditScene> {
        let path = project_path.as_ref()?;
        match Project::load_project(path) {
            Ok(project) => {
                let scene = project.clone().into();
                dep_mut!(session::Session, |session| {
                    session.mark_project_loaded(path.clone(), project);
                });
                Some(scene)
            }
            Err(e) => {
                info!("Failed to load project: {:?}", e);
                None
            }
        }
    }

    fn get_last_project_time() -> Option<SystemTime> {
        let last_project_path = dep_mut!(AutoPersisting<Config>, |config| {
            config
                .read()
                .ok()
                .and_then(|config| config.last_project().cloned())
        })?;

        std::fs::metadata(last_project_path).ok()?.modified().ok()
    }

    /// Get the maximum texture size that was determined during GPU initialization
    fn get_max_texture_size() -> usize {
        let size = MAX_TEXTURE_SIZE.load(Ordering::Relaxed);
        if size > 0 {
            let final_size = size as usize;
            info!("Using GPU-determined max texture size: {}", final_size);
            final_size
        } else {
            // Fallback if GPU limits weren't set (shouldn't happen with wgpu config enabled)
            info!("GPU limits not available, using conservative default of 8192");
            8192
        }
    }
}

impl eframe::App for PhotoBookApp {
    fn update(&mut self, ctx: &Context, _frame: &mut Frame) {
        if !self.loaded_initial_scene {
            egui_extras::install_image_loaders(ctx);

            ctx.input_mut(|input| {
                input.max_texture_side = Self::get_max_texture_size();
            });
            ctx.options_mut(|options| {
                options.reduce_texture_memory = true;
            });

            self.loaded_initial_scene = true;
            self.scene_manager = Self::initialize_scene_manager();
        }

        #[cfg(target_os = "macos")]
        {
            for command in native_mac_menu::drain_commands() {
                self.scene_manager
                    .root_scene
                    .handle_menu_command(command, ctx);
            }
        }

        if !self.loaded_fonts {
            self.loaded_fonts = true;
            // Just load all fonts at start up. Maybe there's a better time to do this?
            dep_mut!(FontManager, |font_manager| {
                font_manager.load_fonts(ctx);
            });
        }

        dep_mut!(CursorManager, |cursor_manager| {
            cursor_manager.begin_frame(ctx);
        });

        // TODO: egui deprecates show() in favor of show_inside(), but show_inside() is for nested UIs.
        // This is a top-level panel, so show() with ctx is still the correct approach.
        // Silencing this warning until the egui API is updated or clarified.
        #[allow(deprecated)]
        egui::CentralPanel::no_frame().show(ctx, |ui| {
            self.scene_manager.ui(ui);

            dep_mut!(ModalManager, |modal_manager| {
                modal_manager.show_next(ui);
            });

            dep_mut!(DeferredWorkManager, |deferred_work_manager| {
                deferred_work_manager.end_frame();
            });
        });

        dep_mut!(CursorManager, |cursor_manager| {
            cursor_manager.end_frame(ctx);
        });

        // Check for pending operations from modals
        if let Some(new_scene) = dep_mut!(session::Session, |session| {
            session.check_modals(&self.scene_manager.root_scene)
        }) {
            self.scene_manager.root_scene = new_scene;
        }

        dep_mut!(AutoSaveManager, |auto_save_manager| {
            let _ = auto_save_manager.auto_save_if_needed(&self.scene_manager.root_scene);
        });
    }

    fn ui(&mut self, _ui: &mut Ui, _frame: &mut Frame) {}
}

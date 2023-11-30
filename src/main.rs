#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use std::{collections::HashSet, path::PathBuf, sync::Arc};

use dependencies::{Dependency, DependencyFor, Singleton, SingletonFor};
use eframe::egui::{self, CentralPanel, Context, ViewportBuilder, Widget};

use photo::Photo;
use photo_manager::{PhotoLoadResult, PhotoManager};
use tokio::runtime;
use widget::{
    image_gallery::{ImageGallery, ImageGalleryResponse},
    image_viewer::{self, ImageViewer, ImageViewerState},
    photo_info::PhotoInfo, page_canvas::{CanvasState, Canvas},
};

use flexi_logger::{Logger, WriteMode};
use string_log::{ArcStringLog, StringLog};

mod dependencies;
mod error_sink;
mod event_bus;
mod image_cache;
mod persistence;
mod photo;
mod photo_manager;
mod string_log;
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

    // Enter the runtime so that `tokio::spawn` is available immediately.
    let _enter = rt.enter();

    let _logger = Logger::try_with_str("info, my::critical::module=trace")
        .unwrap()
        .log_to_writer(Box::new(ArcStringLog::new(Arc::clone(&log))))
        .write_mode(WriteMode::Direct)
        .start()?;

    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default().with_maximized(true),
        ..Default::default()
    };

    let app_log = Arc::clone(&log);

    eframe::run_native(
        "Show an image with eframe/egui",
        options,
        Box::new(|_cc| {
            Box::<MyApp>::new(MyApp {
                photo_manager: Dependency::<PhotoManager>::get(),
                log: app_log,
                nav_stack: vec![AppMode::Gallery {
                    selected_images: HashSet::new(),
                    current_dir: None,
                }],
            })
        }),
    )
    .map_err(|e| anyhow::anyhow!("Error running native app: {}", e))
}

#[derive(Debug, Clone, PartialEq)]
enum AppMode {
    Gallery {
        selected_images: HashSet<PathBuf>,
        current_dir: Option<PathBuf>,
    },
    Viewer {
        photo: Photo,
        index: usize,
        state: ImageViewerState,
    },
    Canvas {
        state: CanvasState,
    }
}

struct MyApp {
    log: Arc<StringLog>,
    photo_manager: Singleton<PhotoManager>,
    nav_stack: Vec<AppMode>,
}

enum NavAction {
    Push(AppMode),
    Pop,
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui_extras::install_image_loaders(ctx);

        let mut nav_action = None;

        match &mut self.nav_stack.last_mut().unwrap() {
            AppMode::Gallery {
                selected_images,
                current_dir,
            } => {
                let mut gallery_response = None;
                egui::CentralPanel::default().show(ctx, |ui: &mut egui::Ui| {
                    gallery_response = ImageGallery::show(ui, current_dir, selected_images);
                });

                if let Some(gallery_response) = gallery_response {
                    match gallery_response {
                        ImageGalleryResponse::ViewPhotoAt(index) => {
                            self.photo_manager.with_lock(|photo_manager| {
                                // TODO: Allow clicking on a pending photo
                                if let PhotoLoadResult::Ready(photo) =
                                    photo_manager.photos[index].clone()
                                {
                                    // self.nav_stack.push(AppMode::Viewer {
                                    //     photo,
                                    //     index,
                                    //     state: ImageViewerState::default(),
                                    // });

                                    self.nav_stack.push(AppMode::Canvas {
                                        state: CanvasState::with_photo(photo),
                                    });
                                }
                            });
                        }
                    }
                }
            }
            AppMode::Viewer {
                photo,
                index,
                state,
            } => {
                let index = index;

                egui::SidePanel::right("viewer_info_panel")
                    .resizable(true)
                    .default_width(100.0)
                    .show(ctx, |ui| {
                        PhotoInfo::new(photo).ui(ui);
                    });

                CentralPanel::default().show(ctx, |ui: &mut egui::Ui| {
                    let viewer_response = ImageViewer::new(photo, state).show(ui);

                    match viewer_response.request {
                        Some(request) => match request {
                            image_viewer::Request::Exit => {
                                nav_action = Some(NavAction::Pop);
                            }
                            image_viewer::Request::Previous => {
                                self.photo_manager.with_lock_mut(|photo_manager| {
                                    let (prev_photo, new_index) =
                                        photo_manager.previous_photo(*index, ctx).unwrap().unwrap();

                                    *photo = prev_photo;
                                    *index = new_index;
                                    *state = ImageViewerState::default();
                                });
                            }
                            image_viewer::Request::Next => {
                                self.photo_manager.with_lock_mut(|photo_manager| {
                                    let (next_photo, new_index) =
                                        photo_manager.next_photo(*index, ctx).unwrap().unwrap();

                                    *photo = next_photo;
                                    *index = new_index;
                                    *state = ImageViewerState::default();
                                });
                            }
                        },
                        None => {}
                    }
                });
            },
            AppMode::Canvas { state } => {
                CentralPanel::default().show(ctx, |ui: &mut egui::Ui| {
                    Canvas::new(state).show(ui);
                });
            }
        };

        if let Some(action) = nav_action {
            match action {
                NavAction::Push(mode) => {
                    self.nav_stack.push(mode);
                }
                NavAction::Pop => {
                    self.nav_stack.pop();
                }
            }
        }
    }
}

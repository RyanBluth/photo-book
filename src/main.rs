#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use std::{collections::HashSet, path::PathBuf, sync::Arc};

use dependencies::{Dependency, DependencyFor, Singleton, SingletonFor};
use eframe::egui::{
    self, CentralPanel, Context, SidePanel, TopBottomPanel, Ui, ViewportBuilder, Widget,
};

use photo::Photo;
use photo_manager::{PhotoLoadResult, PhotoManager};
use tokio::runtime;
use widget::{
    canvas_info::panel::CanvasInfo,
    image_gallery::{ImageGallery, ImageGalleryResponse, ImageGalleryState},
    image_viewer::{self, ImageViewer, ImageViewerState},
    page_canvas::{Canvas, CanvasPhoto, CanvasResponse, CanvasScene, CanvasState},
    photo_info::PhotoInfo,
};

use flexi_logger::{Logger, WriteMode};
use string_log::{ArcStringLog, StringLog};

mod assets;
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

const AUTO_LOAD_PHOTOS: bool = true;

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
        viewport: ViewportBuilder::default()
            .with_maximize_button(true)
            .with_inner_size((3000.0, 2000.0)),
        ..Default::default()
    };

    let app_log = Arc::clone(&log);

    if AUTO_LOAD_PHOTOS {
        PhotoManager::load_directory(PathBuf::from("/home/ryan/Desktop/Aug-5-2023")).unwrap();
    }

    eframe::run_native(
        "Show an image with eframe/egui",
        options,
        Box::new(|_cc| {
            Box::<MyApp>::new(MyApp {
                photo_manager: Dependency::<PhotoManager>::get(),
                log: app_log,
                nav_stack: vec![PrimaryComponent::Gallery {
                    state: ImageGalleryState {
                        selected_images: HashSet::new(),
                        current_dir: None,
                    },
                }],
            })
        }),
    )
    .map_err(|e| anyhow::anyhow!("Error running native app: {}", e))
}

#[derive(Debug, Clone, PartialEq)]
enum PrimaryComponent {
    Gallery {
        state: ImageGalleryState,
    },
    Viewer {
        photo: Photo,
        index: usize,
        state: ImageViewerState,
    },
    Canvas {
        state: CanvasState,
    },
    PhotoInfo {
        photo: Photo,
    },
}

impl PrimaryComponent {
    fn kind(&self) -> PrimaryComponentKind {
        match self {
            PrimaryComponent::Gallery { .. } => PrimaryComponentKind::Gallery,
            PrimaryComponent::Viewer { .. } => PrimaryComponentKind::Viewer,
            PrimaryComponent::Canvas { .. } => PrimaryComponentKind::Canvas,
            PrimaryComponent::PhotoInfo { .. } => PrimaryComponentKind::PhotoInfo,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum PrimaryComponentKind {
    Gallery,
    Viewer,
    Canvas,
    PhotoInfo,
    CanvasInfo,
}

struct MyApp {
    log: Arc<StringLog>,
    photo_manager: Singleton<PhotoManager>,
    nav_stack: Vec<PrimaryComponent>,
}

enum NavAction {
    Push(PrimaryComponent),
    Pop,
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui_extras::install_image_loaders(ctx);

        let component: &mut PrimaryComponent = self.nav_stack.last_mut().unwrap();

        let mut nav_actions = vec![];

        let center_nav_action = MyApp::show_mode(&mut self.photo_manager, ctx, component);

        if let Some(center_nav_action) = center_nav_action {
            nav_actions.push(center_nav_action);
        };

        for action in nav_actions {
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

impl MyApp {
    fn show_mode(
        photo_manager: &mut Singleton<PhotoManager>,
        ctx: &Context,
        mode: &mut PrimaryComponent,
    ) -> Option<NavAction> {
        let mut nav_action = None;

        match mode {
            PrimaryComponent::Gallery { state } => {
                let mut gallery_response = CentralPanel::default()
                    .show(ctx, |ui| ImageGallery::show(ui, state))
                    .inner;

                if let Some(gallery_response) = gallery_response {
                    match gallery_response {
                        ImageGalleryResponse::ViewPhotoAt(index) => {
                            photo_manager.with_lock(|photo_manager| {
                                // TODO: Allow clicking on a pending photo
                                if let PhotoLoadResult::Ready(photo) =
                                    photo_manager.photos[index].clone()
                                {
                                    nav_action = Some(NavAction::Push(PrimaryComponent::Viewer {
                                        photo: photo.clone(),
                                        index,
                                        state: ImageViewerState::default(),
                                    }))
                                }
                            });
                        }
                        ImageGalleryResponse::EditPhotoAt(index) => {
                            photo_manager.with_lock(|photo_manager| {
                                // TODO: Allow clicking on a pending photo
                                if let PhotoLoadResult::Ready(photo) =
                                    photo_manager.photos[index].clone()
                                {
                                    let gallery_state = match mode {
                                        PrimaryComponent::Gallery { state } => state.clone(),
                                        _ => ImageGalleryState::default(),
                                    };

                                    nav_action = Some(NavAction::Push(PrimaryComponent::Canvas {
                                        state: CanvasState::with_photo(photo, gallery_state),
                                    }));
                                }
                            });
                        }
                    }
                }
            }
            PrimaryComponent::Viewer {
                photo,
                index,
                state,
            } => {
                let index = index;

                CentralPanel::default().show(ctx, |ui| {
                    let viewer_response = ImageViewer::new(photo, state).show(ui);
                    match viewer_response.request {
                        Some(request) => match request {
                            image_viewer::Request::Exit => {
                                nav_action = Some(NavAction::Pop);
                            }
                            image_viewer::Request::Previous => {
                                photo_manager.with_lock_mut(|photo_manager| {
                                    let (prev_photo, new_index) = photo_manager
                                        .previous_photo(*index, ui.ctx())
                                        .unwrap()
                                        .unwrap();

                                    *photo = prev_photo;
                                    *index = new_index;
                                    *state = ImageViewerState::default();
                                });
                            }
                            image_viewer::Request::Next => {
                                photo_manager.with_lock_mut(|photo_manager| {
                                    let (next_photo, new_index) = photo_manager
                                        .next_photo(*index, ui.ctx())
                                        .unwrap()
                                        .unwrap();

                                    *photo = next_photo;
                                    *index = new_index;
                                    *state = ImageViewerState::default();
                                });
                            }
                        },
                        None => {}
                    }
                });
            }
            PrimaryComponent::Canvas { state } => match CanvasScene::new(state).show(ctx) {
                Some(request) => match request {
                    CanvasResponse::Exit => {
                        nav_action = Some(NavAction::Pop);
                    }
                },
                None => {}
            },
            PrimaryComponent::PhotoInfo { photo } => {
                SidePanel::right("photo_info_panel").show(ctx, |ui| {
                    PhotoInfo::new(photo).ui(ui);
                });
            }

            _ => {
                todo!()
            }
        }

        nav_action
    }
}

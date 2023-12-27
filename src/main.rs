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
    image_gallery::{ImageGallery, ImageGalleryResponse},
    image_viewer::{self, ImageViewer, ImageViewerState},
    page_canvas::{Canvas, CanvasPhoto, CanvasResponse, CanvasState},
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

    eframe::run_native(
        "Show an image with eframe/egui",
        options,
        Box::new(|_cc| {
            Box::<MyApp>::new(MyApp {
                photo_manager: Dependency::<PhotoManager>::get(),
                log: app_log,
                nav_stack: vec![NavState::new(PrimaryComponent::Gallery {
                    selected_images: HashSet::new(),
                    current_dir: None,
                })],
            })
        }),
    )
    .map_err(|e| anyhow::anyhow!("Error running native app: {}", e))
}

#[derive(Debug, Clone, PartialEq)]
enum PrimaryComponent {
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
}

struct NavState {
    center: Box<PrimaryComponent>,
    left: Option<Box<PrimaryComponent>>,
    right: Option<Box<PrimaryComponent>>,
    bottom: Option<Box<PrimaryComponent>>,
}

impl NavState {
    fn new(center: PrimaryComponent) -> Self {
        Self {
            center: Box::new(center),
            left: None,
            right: None,
            bottom: None,
        }
    }
}

struct MyApp {
    log: Arc<StringLog>,
    photo_manager: Singleton<PhotoManager>,
    nav_stack: Vec<NavState>,
}

enum NavAction {
    Push(NavState),
    Pop,
    OpenPhoto(Photo),
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui_extras::install_image_loaders(ctx);

        let nav_state: &mut NavState = self.nav_stack.last_mut().unwrap();

        let mut nav_actions = vec![];

        if let Some(left_state) = nav_state.left.as_mut() {
            let left_nav_action = SidePanel::left("split_left_panel")
                .resizable(true)
                .default_width(400.0)
                .show(ctx, |ui| {
                    MyApp::show_mode(&mut self.photo_manager, ui, left_state)
                })
                .inner;

            if let Some(left_nav_action) = left_nav_action {
                nav_actions.push(left_nav_action);
            }
        }

        if let Some(right_state) = nav_state.right.as_mut() {
            let right_nav_action = SidePanel::right("split_right_panel")
                .resizable(true)
                .default_width(400.0)
                .show(ctx, |ui| {
                    MyApp::show_mode(&mut self.photo_manager, ui, right_state)
                })
                .inner;
            if let Some(right_nav_action) = right_nav_action {
                nav_actions.push(right_nav_action);
            }
        }

        if let Some(bottom_state) = nav_state.bottom.as_mut() {
            let bottom_nav_action = TopBottomPanel::bottom("split_bottom_panel")
                .resizable(true)
                .default_height(400.0)
                .show(ctx, |ui| {
                    MyApp::show_mode(&mut self.photo_manager, ui, bottom_state)
                })
                .inner;
            if let Some(bottom_nav_action) = bottom_nav_action {
                nav_actions.push(bottom_nav_action);
            }
        }

        let center_nav_action = CentralPanel::default()
            .show(ctx, |ui| {
                MyApp::show_mode(&mut self.photo_manager, ui, nav_state.center.as_mut())
            })
            .inner;

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
                NavAction::OpenPhoto(photo) => {
                    let current = self.nav_stack.last_mut().unwrap();

                    if current.center.kind() == PrimaryComponentKind::Canvas {
                        if let PrimaryComponent::Canvas { state } = current.center.as_mut() {
                            state.add_photo(photo);
                        }
                    } else {
                        let current = self.nav_stack.pop().unwrap();
                        self.nav_stack.push(NavState {
                            center: Box::new(PrimaryComponent::Canvas {
                                state: CanvasState::with_photo(photo),
                            }),
                            left: Some(current.center),
                            right: None,
                            bottom: None,
                        });
                    }
                }
            }
        }
    }
}

impl MyApp {
    fn show_mode(
        photo_manager: &mut Singleton<PhotoManager>,
        ui: &mut Ui,
        mode: &mut PrimaryComponent,
    ) -> Option<NavAction> {
        let mut nav_action = None;

        match mode {
            PrimaryComponent::Gallery {
                selected_images,
                current_dir,
            } => {
                let mut gallery_response = ImageGallery::show(ui, current_dir, selected_images);

                if let Some(gallery_response) = gallery_response {
                    match gallery_response {
                        ImageGalleryResponse::ViewPhotoAt(index) => {
                            photo_manager.with_lock(|photo_manager| {
                                // TODO: Allow clicking on a pending photo
                                if let PhotoLoadResult::Ready(photo) =
                                    photo_manager.photos[index].clone()
                                {
                                    nav_action = Some(NavAction::Push(NavState {
                                        center: Box::new(PrimaryComponent::Viewer {
                                            photo: photo.clone(),
                                            index,
                                            state: ImageViewerState::default(),
                                        }),
                                        left: None,
                                        right: Some(Box::new(PrimaryComponent::PhotoInfo {
                                            photo,
                                        })),
                                        bottom: None,
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
                                    nav_action = Some(NavAction::OpenPhoto(photo));
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
                                let (next_photo, new_index) =
                                    photo_manager.next_photo(*index, ui.ctx()).unwrap().unwrap();

                                *photo = next_photo;
                                *index = new_index;
                                *state = ImageViewerState::default();
                            });
                        }
                    },
                    None => {}
                }
            }
            PrimaryComponent::Canvas { state } => match Canvas::new(state).show(ui) {
                Some(request) => match request {
                    CanvasResponse::Exit => {
                        nav_action = Some(NavAction::Pop);
                    }
                },
                None => {}
            },
            PrimaryComponent::PhotoInfo { photo } => {
                PhotoInfo::new(photo).ui(ui);
            }
            _ => {
                todo!()
            }
        }

        nav_action
    }
}

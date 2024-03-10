#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use std::{borrow::BorrowMut, collections::HashSet, sync::Arc};

use cursor_manager::CursorManager;
use dependencies::{Dependency, DependencyFor, Singleton, SingletonFor};
use eframe::egui::{self, CentralPanel, Context, SidePanel, ViewportBuilder, Widget};

use egui::{accesskit::Tree, menu};
use egui_tiles::{Behavior, Tile, UiResponse};
use font_manager::FontManager;
use log::info;
use photo::Photo;
use photo_manager::{PhotoLoadResult, PhotoManager};
use scene::{SceneManager, SceneState};
use tokio::runtime;
use widget::{
    gallery_image::GalleryImage,
    image_gallery::{ImageGallery, ImageGalleryResponse, ImageGalleryState},
    image_viewer::{self, ImageViewer, ImageViewerState},
    page_canvas::{CanvasResponse, CanvasScene, CanvasState},
    photo_info::PhotoInfo,
};

use flexi_logger::{Logger, WriteMode};
use string_log::{ArcStringLog, StringLog};

mod assets;
mod cursor_manager;
mod dependencies;
mod error_sink;
mod font_manager;
mod history;
mod persistence;
mod photo;
mod photo_manager;
mod scene;
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
        Box::new(|_cc| Box::<PhotoBookApp>::new(PhotoBookApp::new(app_log))),
    )
    .map_err(|e| anyhow::anyhow!("Error running native app: {}", e))
}

struct TreeBehavior {}

impl egui_tiles::Behavior<PrimaryComponent> for TreeBehavior {
    fn tab_title_for_pane(&mut self, component: &PrimaryComponent) -> egui::WidgetText {
        component.title().into()
    }

    fn pane_ui(
        &mut self,
        ui: &mut egui::Ui,
        _tile_id: egui_tiles::TileId,
        component: &mut PrimaryComponent,
    ) -> egui_tiles::UiResponse {
        let photo_manager: Singleton<PhotoManager> = Dependency::get();
        let mut nav_action = None;

        match component {
            PrimaryComponent::Gallery { state } => {
                let gallery_response = ImageGallery::show(ui, state);

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
                                    let gallery_state = match component {
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
            // PrimaryComponent::Canvas { state } => match CanvasScene::new(state).show(ctx) {
            //     Some(request) => match request {
            //         CanvasResponse::Exit => {
            //             nav_action = Some(NavAction::Pop);
            //         }
            //     },
            //     None => {}
            // },
            PrimaryComponent::PhotoInfo { photo } => {
                PhotoInfo::new(photo).ui(ui);
            }

            _ => {
                todo!()
            }
        }

        return egui_tiles::UiResponse::None;
    }
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
    fn title(&self) -> String {
        match self {
            PrimaryComponent::Gallery { .. } => "Gallery".to_string(),
            PrimaryComponent::Viewer { .. } => "Viewer".to_string(),
            PrimaryComponent::Canvas { .. } => "Canvas".to_string(),
            PrimaryComponent::PhotoInfo { .. } => "Photo Info".to_string(),
        }
    }
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
    Gallery = 0,
    Viewer = 1,
    Canvas = 2,
    PhotoInfo = 3,
    CanvasInfo = 4,
}

struct PhotoBookApp {
    log: Arc<StringLog>,
    photo_manager: Singleton<PhotoManager>,
    nav_stack: Vec<PrimaryComponent>,
    loaded_fonts: bool,
    scene_manager: SceneManager,
}

impl PhotoBookApp {
    fn new(log: Arc<StringLog>) -> Self {
        Self {
            photo_manager: Dependency::<PhotoManager>::get(),
            log: log,
            nav_stack: vec![PrimaryComponent::Gallery {
                state: ImageGalleryState {
                    selected_images: HashSet::new(),
                    current_dir: None,
                },
            }],
            loaded_fonts: false,
            scene_manager: SceneManager::default(),
        }
    }
}

enum NavAction {
    Push(PrimaryComponent),
    Pop,
}

impl eframe::App for PhotoBookApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui_extras::install_image_loaders(ctx);

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

        let component: &mut PrimaryComponent = self.nav_stack.last_mut().unwrap();

        let mut nav_actions = vec![];

        egui::CentralPanel::default().show(ctx, |ui| {
            self.scene_manager.ui(ui);
        });

        // if let Some(center_nav_action) = center_nav_action {
        //     nav_actions.push(center_nav_action);
        // };

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

        Dependency::<CursorManager>::get().with_lock_mut(|cursor_manager| {
            cursor_manager.end_frame(ctx);
        });
    }
}

impl PhotoBookApp {
    fn show_mode(
        photo_manager: &mut Singleton<PhotoManager>,
        ctx: &Context,
        mode: &mut PrimaryComponent,
    ) -> Option<NavAction> {
        let mut nav_action = None;

        match mode {
            PrimaryComponent::Gallery { state } => {
                let gallery_response = CentralPanel::default()
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

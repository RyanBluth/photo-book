#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use std::{fs::read_dir, path::PathBuf, sync::Arc, collections::HashSet};

use dependencies::{Dependency, DependencyFor, Singleton, SingletonFor};
use eframe::{
    egui::{self, load::SizedTexture, menu, CentralPanel, Image, Key, Layout, ScrollArea, Widget},
    emath::Align,
    epaint::{Pos2, Rect, Vec2},
};
use egui_extras::Column;
use event_bus::{EventBus, EventBusId, GalleryImageEvent};
use log::{error, info};
use photo::Photo;
use photo_manager::PhotoManager;
use widget::{
    gallery_image::GalleryImage,
    image_viewer::{self, ImageViewer, ImageViewerState},
    photo_info::PhotoInfo,
    spacer::Spacer,
};

use flexi_logger::{Logger, WriteMode};
use string_log::{ArcStringLog, StringLog};

mod dependencies;
mod event_bus;
mod image_cache;
mod photo;
mod photo_manager;
mod string_log;
mod utils;
mod widget;
mod persistence;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let log: Arc<StringLog> = Arc::new(StringLog::new());

    rayon::ThreadPoolBuilder::new()
        .num_threads(4)
        .build_global()
        .unwrap();

    let _logger = Logger::try_with_str("info, my::critical::module=trace")
        .unwrap()
        .log_to_writer(Box::new(ArcStringLog::new(Arc::clone(&log))))
        .write_mode(WriteMode::Direct)
        .start()?;

    let options = eframe::NativeOptions {
        initial_window_size: Some(egui::vec2(900.0, 900.0)),
        ..Default::default()
    };

    let app_log = Arc::clone(&log);

    eframe::run_native(
        "Show an image with eframe/egui",
        options,
        Box::new(|_cc| {
            Box::<MyApp>::new(MyApp {
                current_dir: None,
                photo_manager: Dependency::<PhotoManager>::get(),
                log: app_log,
                mode: AppMode::Gallery {
                    selected_images: HashSet::new(),
                },
            })
        }),
    )
    .map_err(|e| anyhow::anyhow!("Error running native app: {}", e))
}

#[derive(Debug, Clone)]
enum AppMode {
    Gallery {
        selected_images: HashSet<String>,
    },
    Viewer {
        photo: Photo,
        index: usize,
        state: ImageViewerState,
    },
}

struct MyApp {
    current_dir: Option<PathBuf>,
    log: Arc<StringLog>,
    photo_manager: Singleton<PhotoManager>,
    mode: AppMode,
}

impl eframe::App for MyApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui_extras::install_image_loaders(&ctx);

        // TODO: Don't clone if possible
        match self.mode.clone() {
            AppMode::Gallery {
                mut selected_images,
            } => { 
                self.gallery(ctx, &mut selected_images);
                self.mode = AppMode::Gallery {
                    selected_images,
                };
            },
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
                        PhotoInfo::new(&photo).ui(ui);
                    });

                CentralPanel::default().show(ctx, |ui| {
                    let mut state = state.clone();

                    let viewer_response = ImageViewer::new(&photo, &mut state).show(ui);

                    match viewer_response.request {
                        Some(request) => match request {
                            image_viewer::Request::Exit => {
                                self.mode = AppMode::Gallery {
                                    selected_images: HashSet::new(),
                                };
                            }
                            image_viewer::Request::Previous => {
                                self.photo_manager.with_lock_mut(|photo_manager| {
                                    let (prev_photo, new_index) =
                                        photo_manager.previous_photo(index, ctx).unwrap().unwrap();

                                    self.mode = AppMode::Viewer {
                                        photo: prev_photo,
                                        index: new_index,
                                        state: ImageViewerState::default(),
                                    };
                                });
                            }
                            image_viewer::Request::Next => {
                                self.photo_manager.with_lock_mut(|photo_manager| {
                                    let (next_photo, new_index) =
                                        photo_manager.next_photo(index, ctx).unwrap().unwrap();

                                    self.mode = AppMode::Viewer {
                                        photo: next_photo,
                                        index: new_index,
                                        state: ImageViewerState::default(),
                                    };
                                });
                            }
                        },
                        None => {
                            self.mode = AppMode::Viewer {
                                photo,
                                index,
                                state,
                            };
                        }
                    }
                });
            }
        }
    }
}

impl MyApp {
    fn gallery(&mut self, ctx: &egui::Context, selected_images: &mut HashSet<String>) {
        egui::TopBottomPanel::bottom("log")
            .resizable(true)
            .show(ctx, |ui| {
                ui.with_layout(
                    Layout {
                        main_dir: egui::Direction::TopDown,
                        main_wrap: false,
                        main_align: Align::Min,
                        main_justify: false,
                        cross_align: Align::Min,
                        cross_justify: true,
                    },
                    |ui| {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            self.log.for_each(|line| {
                                //ui.label(line);
                            });
                        });
                    },
                );
            });

        egui::CentralPanel::default().show(ctx, |ui| {
            menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Open").clicked() {
                        self.current_dir = native_dialog::FileDialog::new()
                            .add_filter("Images", &["png", "jpg", "jpeg"])
                            .show_open_single_dir()
                            .unwrap();

                        info!("Opened {:?}", self.current_dir);

                        self.photo_manager.with_lock_mut(|photo_manager| {
                            photo_manager.load_directory(&self.current_dir.as_ref().unwrap(), ctx);
                        });
                    }
                });
            });

            match self.current_dir {
                Some(ref path) => {
                    ui.label(format!("Current Dir: {}", path.display()));

                    if ui.input(|input| input.key_down(Key::Escape)) {
                        selected_images.clear();
                    }

                    ui.spacing_mut().item_spacing = Vec2::splat(10.0);

                    let window_width = ui.available_width();
                    let window_height = ui.available_height();
                    let column_width = 256.0;
                    let row_height = 256.0;
                    let num_columns: usize = (window_width / column_width).floor() as usize;

                    //let padding_size = num_columns as f32 * 10.0;
                    let spacer_width = (window_width
                        - ((column_width + ui.spacing().item_spacing.x) * num_columns as f32)
                        - 10.0
                        - ui.spacing().item_spacing.x)
                        .max(0.0);

                    let num_photos = self
                        .photo_manager
                        .with_lock(|photo_manager| photo_manager.photos.len());

                    let num_rows = num_photos.div_ceil(num_columns);

                    egui_extras::TableBuilder::new(ui)
                        .min_scrolled_height(window_height)
                        .columns(Column::exact(column_width), num_columns)
                        .column(Column::exact(spacer_width))
                        .body(|body| {
                            body.rows(row_height, num_rows, |row_idx, mut row| {
                                let offest = row_idx * num_columns;
                                for i in 0..num_columns {

                                    if offest + i >= num_photos {
                                        break;
                                    }

                                    row.col(|ui| {
                                        self.photo_manager.with_lock_mut(|photo_manager| {

                                            let photo = photo_manager.photos[offest + i].clone();
                                            
                                            let image = GalleryImage::new(
                                                photo.clone(),
                                                photo_manager.tumbnail_texture_at(offest + i, ctx),
                                                selected_images.iter().filter(|path| path == &&photo.path.display().to_string()).count() > 0,
                                            );

                                            let image_response = ui.add(image);

                                            if image_response.clicked() {
                                                let ctrl_held = ui.input(|input| input.modifiers.ctrl);
                                                if ctrl_held {
                                                    if selected_images.iter().filter(|path| path == &&photo.path.display().to_string()).count() > 0 {
                                                        selected_images.remove(&photo.path.display().to_string());
                                                    } else {
                                                        selected_images.insert(photo.path.display().to_string());
                                                    }
                                                } else {
                                                    selected_images.clear();
                                                    selected_images.insert(photo.path.display().to_string());
                                                }
                                            }

                                            if image_response.double_clicked() {
                                                self.mode = AppMode::Viewer {
                                                    photo: photo_manager.photos[offest + i].clone(),
                                                    index: offest + i,
                                                    state: ImageViewerState::default(),
                                                };
                                            }
                                        });
                                    });
                                }

                                row.col(|ui| {
                                    ui.add(Spacer::new(spacer_width, row_height));
                                });
                            })
                        });
                }
                None => {
                    ui.label("No folder selected");
                }
            }
        });
    }
}

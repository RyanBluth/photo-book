#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use std::{fs::read_dir, path::PathBuf, sync::Arc};

use dependencies::{Dependency, DependencyFor, Singleton, SingletonFor};
use eframe::{
    egui::{self, load::SizedTexture, CentralPanel, Image, Key, Layout, ScrollArea, Widget},
    emath::Align,
    epaint::{Pos2, Rect, Vec2},
};
use egui_extras::Column;
use event_bus::{EventBus, EventBusId, GalleryImageEvent};
use gallery_service::ThumbnailService;
use log::{error, info};
use photo::Photo;
use photo_manager::PhotoManager;
use widget::{
    gallery_image::GalleryImage,
    image_viewer::{self, ImageViewer, ImageViewerState},
    spacer::Spacer, photo_info::PhotoInfo,
};

use flexi_logger::{Logger, WriteMode};
use string_log::{ArcStringLog, StringLog};

mod dependencies;
mod event_bus;
mod gallery_service;
mod image_cache;
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
                mode: AppMode::Gallery,
            })
        }),
    )
    .map_err(|e| anyhow::anyhow!("Error running native app: {}", e))
}

#[derive(Debug, Clone)]
enum AppMode {
    Gallery,
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
            AppMode::Gallery => self.gallery(ctx),
            AppMode::Viewer {
                photo,
                index,
                state,
            } => {
                let index = index;

                egui::SidePanel::right("viewer_info_panel").resizable(true).show(ctx, |ui| {
                    PhotoInfo::new(&photo).ui(ui);
                });

                CentralPanel::default().show(ctx, |ui| {
                    let mut state = state.clone();

                    let viewer_response =
                        ImageViewer::new(&photo, &mut state)
                            .show(ui);

                    match viewer_response.request {
                        Some(request) => match request {
                            image_viewer::Request::Exit => {
                                self.mode = AppMode::Gallery;
                            }
                            image_viewer::Request::Previous => {
                                self.photo_manager.with_lock_mut(|photo_manager| {
                                    let (prev_photo, new_index) = photo_manager
                                        .previous_photo(index, ctx)
                                        .unwrap()
                                        .unwrap();

                                    self.mode = AppMode::Viewer {
                                        photo: prev_photo,
                                        index: new_index,
                                        state: ImageViewerState::default(),
                                    };
                                });
                            }
                            image_viewer::Request::Next => {
                                self.photo_manager.with_lock_mut(|photo_manager| {
                                    let (next_photo, new_index) = photo_manager.next_photo(index, ctx).unwrap().unwrap();

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
    fn gallery(&mut self, ctx: &egui::Context) {
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
            ScrollArea::both().show(ui, |ui| {
                let button = ui.button("Open Folder");

                if button.clicked() {
                    self.current_dir = native_dialog::FileDialog::new()
                        .add_filter("Images", &["png", "jpg", "jpeg"])
                        .show_open_single_dir()
                        .unwrap();

                    info!("Opened {:?}", self.current_dir);

                    self.photo_manager.with_lock_mut(|photo_manager| {
                        photo_manager.load_directory(&self.current_dir.as_ref().unwrap(), ctx);
                    });
                }

                match self.current_dir {
                    Some(ref path) => {
                        ui.label(format!("Current Dir: {}", path.display()));

                        let window_width = ui.available_width();
                        let window_height = ui.available_height();
                        let column_width = 256.0;
                        let row_height = 256.0;
                        let num_columns: usize = (window_width / column_width).floor() as usize;

                        //let padding_size = num_columns as f32 * 10.0;
                        let spacer_width =
                            (window_width - (column_width * num_columns as f32) - 10.0).max(0.0);

                        ui.spacing_mut().item_spacing.x = 0.0;

                        let num_photos = self
                            .photo_manager
                            .with_lock(|photo_manager| photo_manager.photos.len());

                        egui_extras::TableBuilder::new(ui)
                            .min_scrolled_height(window_height)
                            .columns(Column::exact(column_width), num_columns)
                            .column(Column::exact(spacer_width))
                            .body(|body| {
                                body.rows(
                                    row_height,
                                    num_photos / num_columns,
                                    |row_idx, mut row| {
                                        let offest = row_idx * num_columns;
                                        for i in 0..num_columns {
                                            row.col(|ui| {
                                                self.photo_manager.with_lock_mut(|photo_manager| {
                                                    let image = GalleryImage::new(
                                                        photo_manager.photos[offest + i].clone(),
                                                        photo_manager
                                                            .tumbnail_texture_at(offest + i, ctx),
                                                    );

                                                    if ui.add(image).clicked() {
                                                        self.mode = AppMode::Viewer {
                                                            photo: photo_manager.photos[offest + i]
                                                                .clone(),
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
                                    },
                                )
                            });
                    }
                    None => {
                        ui.label("No folder selected");
                    }
                }
            });
        });
    }
}

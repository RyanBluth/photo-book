#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use std::{fs::read_dir, path::PathBuf, sync::Arc};

use dependencies::{Dependency, DependencyFor, SingletonFor};
use eframe::{
    egui::{self, CentralPanel, Image, Key, Layout, ScrollArea},
    emath::Align,
    epaint::{Pos2, Rect, Vec2},
};
use egui_extras::Column;
use event_bus::{EventBus, EventBusId, GalleryImageEvent};
use gallery_service::ThumbnailService;
use log::{error, info};
use photo::Photo;
use widget::{
    gallery_image::GalleryImage,
    image_viewer::{self, ImageViewer, ImageViewerState},
    spacer::Spacer,
};

use flexi_logger::{Logger, WriteMode};
use string_log::{ArcStringLog, StringLog};

mod dependencies;
mod event_bus;
mod gallery_service;
mod image_cache;
mod photo;
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
                images: Vec::new(),
                log: app_log,
                thumbnail_service: Dependency::<ThumbnailService>::get(),
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
    images: Vec<Photo>,
    log: Arc<StringLog>,
    thumbnail_service: ThumbnailService,
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
                // } => self.viewer(ctx, photo.clone(), *index, *scale, *offset),
            } => {
                let index = index;
                CentralPanel::default().show(ctx, |ui| {
                    let mut state = state.clone();

                    let viewer_response = ImageViewer::new(&photo, &mut state).show(ui);

                    match viewer_response.request {
                        Some(request) => match request {
                            image_viewer::Request::Exit => {
                                self.mode = AppMode::Gallery;
                            }
                            image_viewer::Request::Previous => {
                                self.mode = AppMode::Viewer {
                                    photo: self.images
                                        [(index + self.images.len() - 1) % self.images.len()]
                                    .clone(),
                                    index: (index + self.images.len() - 1) % self.images.len(),
                                    state: ImageViewerState::default(),
                                };
                            }
                            image_viewer::Request::Next => {
                                self.mode = AppMode::Viewer {
                                    photo: self.images[(index + 1) % self.images.len()].clone(),
                                    index: (index + 1) % self.images.len(),
                                    state: ImageViewerState::default(),
                                };
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
                                ui.label(line);
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

                    let entries: Vec<Result<std::fs::DirEntry, std::io::Error>> =
                        read_dir(self.current_dir.as_ref().unwrap())
                            .unwrap()
                            .collect();

                    for entry in entries {
                        let entry = entry.as_ref().unwrap();
                        let path = entry.path();

                        if path.extension().unwrap_or_default().to_ascii_lowercase() != "jpg" {
                            continue;
                        }
                        self.images.push(Photo::new(path));
                    }

                    match self
                        .thumbnail_service
                        .gen_thumbnails(self.current_dir.as_ref().unwrap().clone(), ctx.clone())
                    {
                        Ok(_) => {}
                        Err(error) => {
                            println!("Failed to generate thumbnails {}", error);
                            error!("Failed to generate thumbnails {}", error);
                        }
                    }
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

                        egui_extras::TableBuilder::new(ui)
                            .min_scrolled_height(window_height)
                            .columns(Column::exact(column_width), num_columns)
                            .column(Column::exact(spacer_width))
                            .body(|body| {
                                body.rows(
                                    row_height,
                                    self.images.len() / num_columns,
                                    |row_idx, mut row| {
                                        let offest = row_idx * num_columns;
                                        for i in 0..num_columns {
                                            row.col(|ui| {
                                                let image = GalleryImage::new(
                                                    self.images[offest + i].clone(),
                                                );

                                                if ui.add(image).clicked() {
                                                    self.mode = AppMode::Viewer {
                                                        photo: self.images[offest + i].clone(),
                                                        index: offest + i,
                                                        state: ImageViewerState::default(),
                                                    };
                                                }
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

    fn viewer(
        &mut self,
        ctx: &egui::Context,
        photo: Photo,
        index: usize,
        scale: f32,
        offset: Vec2,
    ) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical(|ui| {
                let button = ui.button("Back");

                if button.clicked() {
                    self.mode = AppMode::Gallery;
                }

                let window_width = ui.available_width();
                let window_height = ui.available_height();

                ui.horizontal_centered(|ui| {
                    let scroll_area = ScrollArea::both().scroll_offset(offset).show(ui, |ui| {
                        ui.add(
                            Image::from_uri(photo.uri())
                                .maintain_aspect_ratio(true)
                                .fit_to_exact_size(Vec2::new(
                                    window_width * scale,
                                    window_height * scale,
                                )),
                        );
                    });
                });
            });
        });
    }
}

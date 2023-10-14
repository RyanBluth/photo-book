#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")] // hide console window on Windows in release

use std::{
    fs::read_dir,
    io::{BufReader, Read},
    path::PathBuf,
    sync::Arc, hash::Hash,
};

use async_retained_image::AsyncRetainedImage;
use eframe::{
    egui::{self, Layout, ScrollArea},
    emath::Align,
    epaint::{
        Vec2,
    },
};
use egui_extras::RetainedImage;
use gallery_service::GalleryService;
use log::info;
use rayon::{slice::ParallelSlice, string};

use flexi_logger::{FileSpec, Logger, WriteMode};
use string_log::{ArcStringLog, StringLog};

mod async_retained_image;
mod gallery_service;
mod string_log;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let log: Arc<StringLog> = Arc::new(StringLog::new());

    rayon::ThreadPoolBuilder::new().num_threads(4).build_global().unwrap();

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
            })
        }),
    )
    .map_err(|e| anyhow::anyhow!("Error running native app: {}", e))
}

struct MyApp {
    current_dir: Option<PathBuf>,
    images: Vec<PathBuf>,
    log: Arc<StringLog>,
}

impl eframe::App for MyApp {

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {

        egui_extras::install_image_loaders(&ctx);

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
                        self.images.push(path);
                    }

                    GalleryService::gen_thumbnails(self.current_dir.as_ref().unwrap().clone());
                }

                match self.current_dir {
                    Some(ref path) => {
                        ui.label(format!("Current Dir: {}", path.display()));

                        egui::Grid::new("some_unique_id").show(ui, |ui| {
                            let mut count = 0;
                            for path in &self.images {
                                // image.show_size(ui, Vec2::new(200.0, 200.0));

                                ui.add(AsyncRetainedImage::new(path.clone()));

                                count += 1;

                                if count >= 11 {
                                    ui.end_row();
                                    count = 0;
                                }
                            }
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

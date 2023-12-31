use std::{collections::HashSet, path::PathBuf};

use eframe::{
    egui::{self, menu, Key, Layout, Ui},
    emath::Align,
    epaint::Vec2,
};
use egui_extras::Column;

use log::info;

use crate::{
    dependencies::{Dependency, Singleton, SingletonFor},
    photo::Photo,
    photo_manager::PhotoManager,
};

use super::{gallery_image::GalleryImage, page_canvas::TransformableWidget, spacer::Spacer};

pub struct ImageGallery<'a> {
    current_dir: Option<std::path::PathBuf>,
    photo_manager: Singleton<PhotoManager>,
    selected_images: &'a mut HashSet<Photo>,
}

pub enum Request {
    LoadDirectory(std::path::PathBuf),
    ViewImageAt(usize),
}

pub enum ImageGalleryResponse {
    ViewPhotoAt(usize),
    EditPhotoAt(usize),
}

impl<'a> ImageGallery<'a> {
    pub fn show(
        ui: &mut Ui,
        current_dir: &mut Option<std::path::PathBuf>,
        selected_images: &'a mut HashSet<PathBuf>,
    ) -> Option<ImageGalleryResponse> {
        let mut response = None;
        let photo_manager: Singleton<PhotoManager> = Dependency::get();

        menu::bar(ui, |ui| {
            ui.menu_button("File", |ui| {
                if ui.button("Open").clicked() {
                    *current_dir = native_dialog::FileDialog::new()
                        .add_filter("Images", &["png", "jpg", "jpeg"])
                        .show_open_single_dir()
                        .unwrap();

                    info!("Opened {:?}", current_dir);

                    PhotoManager::load_directory(current_dir.clone().unwrap());
                }
            });
        });

        match current_dir {
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

                let num_photos =
                    photo_manager.with_lock(|photo_manager| photo_manager.photos.len());

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
                                    photo_manager.with_lock_mut(|photo_manager| {
                                        let photo = photo_manager.photos[offest + i].clone();

                                        let image = GalleryImage::new(
                                            photo.clone(),
                                            photo_manager.tumbnail_texture_at(offest + i, ui.ctx()),
                                            selected_images.contains(photo.path()),
                                        );

                                        let image_response = ui.add(image);

                                        if image_response.clicked() {
                                            let ctrl_held = ui.input(|input| input.modifiers.ctrl);
                                            if ctrl_held {
                                                if selected_images.contains(photo.path()) {
                                                    selected_images.remove(photo.path());
                                                } else {
                                                    selected_images.insert(photo.path().clone());
                                                }
                                            } else {
                                                selected_images.clear();
                                                selected_images.insert(photo.path().clone());
                                            }
                                        }

                                        if image_response.double_clicked() {
                                            response =
                                                Some(ImageGalleryResponse::ViewPhotoAt(offest + i));
                                        } else if image_response.middle_clicked() {
                                            response =
                                                Some(ImageGalleryResponse::EditPhotoAt(offest + i));
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

        response
    }
}

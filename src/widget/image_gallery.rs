use std::{collections::HashSet, path::PathBuf};

use eframe::{
    egui::{menu, Key, Ui},
    epaint::Vec2,
};
use egui_extras::Column;

use log::info;

use crate::{
    dependencies::{Dependency, Singleton, SingletonFor},
    photo_manager::PhotoManager,
};

use super::{gallery_image::GalleryImage, spacer::Spacer};

#[derive(Debug, PartialEq, Clone, Default)]
pub struct ImageGalleryState {
    pub selected_images: HashSet<PathBuf>,
    pub current_dir: Option<PathBuf>,
}

pub struct ImageGallery<'a> {
    photo_manager: Singleton<PhotoManager>,
    state: &'a mut ImageGalleryState,
}

pub enum ImageGalleryResponse {
    ViewPhotoAt(usize),
    EditPhotoAt(usize),
}

impl<'a> ImageGallery<'a> {
    pub fn show(ui: &mut Ui, state: &'a mut ImageGalleryState) -> Option<ImageGalleryResponse> {
        let mut response = None;
        let photo_manager: Singleton<PhotoManager> = Dependency::get();

        let current_dir = &mut state.current_dir;
        let selected_images = &mut state.selected_images;

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
                        body.rows(row_height, num_rows, |mut row| {
                            let offest = row.index() * num_columns;
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

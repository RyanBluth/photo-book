use std::{collections::HashSet, path::PathBuf};

use eframe::{
    egui::{Key, Ui},
    epaint::Vec2,
};

use egui::{text::LayoutJob, Color32, Image, Layout, Slider};
use egui_extras::Column;

use crate::{
    assets::Asset,
    dependencies::{Dependency, Singleton, SingletonFor},
    photo::Photo,
    photo_manager::PhotoManager,
    utils::EguiUiExt,
};

use super::{gallery_image::GalleryImage, spacer::Spacer};

#[derive(Debug, PartialEq, Clone)]
pub struct ImageGalleryState {
    pub selected_images: HashSet<PathBuf>,
    pub scale: f32,
}

impl Default for ImageGalleryState {
    fn default() -> Self {
        Self {
            selected_images: HashSet::new(),
            scale: 1.0,
        }
    }
}

pub struct ImageGallery<'a> {
    photo_manager: Singleton<PhotoManager>,
    state: &'a mut ImageGalleryState,
}

pub enum ImageGalleryResponse {
    SelectPhotoPrimaryAction(Photo),
    SelectPhotoSecondaryAction(Photo),
}

impl<'a> ImageGallery<'a> {
    pub fn show(ui: &mut Ui, state: &'a mut ImageGalleryState) -> Option<ImageGalleryResponse> {
        let mut response = None;
        let photo_manager: Singleton<PhotoManager> = Dependency::get();
        let selected_images = &mut state.selected_images;

        let has_photos = photo_manager.with_lock(|photo_manager| !photo_manager.photos.is_empty());

        if has_photos {
            ui.vertical(|ui| {
                if ui.input(|input| input.key_down(Key::Escape)) {
                    selected_images.clear();
                }

                let spacing = 10.0;

                let bottom_bar_height = 50.0;

                let mut table_size = ui.available_size();
                table_size.y -= bottom_bar_height;

                ui.allocate_ui(table_size, |ui| {
                    ui.spacing_mut().item_spacing = Vec2::splat(spacing);

                    let column_width: f32 = 256.0 * state.scale;
                    let row_height = 256.0 * state.scale;
                    let num_columns: usize =
                        (table_size.x / (column_width + spacing)).floor().max(1.0) as usize;

                    let spacer_width = (table_size.x
                        - ((column_width + ui.spacing().item_spacing.x) * num_columns as f32)
                        - 10.0
                        - ui.spacing().item_spacing.x)
                        .max(0.0);

                    let grouped_photos = photo_manager
                        .with_lock_mut(|photo_manager| photo_manager.grouped_photos().clone());

                    struct RowMetadata {
                        height: f32,
                        is_title: bool,
                        section: String,
                        row_index_in_section: usize,
                    }

                    let row_metadatas: Vec<RowMetadata> = {
                        grouped_photos
                            .iter()
                            .flat_map(|(title, group)| {
                                let rows = group.len().div_ceil(num_columns);

                                let mut metadatas: Vec<RowMetadata> = vec![RowMetadata {
                                    height: 30.0,
                                    is_title: true,
                                    section: title.clone(),
                                    row_index_in_section: 0,
                                }];

                                for row_idx in 0..rows {
                                    metadatas.push(RowMetadata {
                                        height: row_height,
                                        is_title: false,
                                        section: title.clone(),
                                        row_index_in_section: row_idx,
                                    });
                                }

                                metadatas
                            })
                            .collect()
                    };

                    let heights: Vec<f32> = row_metadatas.iter().map(|x| x.height).collect();

                    egui_extras::TableBuilder::new(ui)
                        .min_scrolled_height(table_size.y)
                        .auto_shrink(false)
                        .columns(Column::exact(column_width), num_columns)
                        .column(Column::exact(spacer_width))
                        .body(|body| {
                            body.heterogeneous_rows(heights.into_iter(), |mut row| {
                                let row_index = row.index();
                                let metadata = &row_metadatas[row_index];
                                let offest = metadata.row_index_in_section * num_columns;

                                let group = grouped_photos.get(&metadata.section).unwrap();

                                if metadata.is_title {
                                    row.col(|ui| {
                                        ui.vertical(|ui| {
                                            ui.add_space(10.0);
                                            ui.heading(metadata.section.clone());
                                        });
                                    });
                                } else {
                                    for i in 0..num_columns {
                                        if offest + i >= group.len() {
                                            break;
                                        }

                                        row.col(|ui: &mut Ui| {
                                            let photo = &group[offest + i];
                                            photo_manager.with_lock_mut(|photo_manager| {
                                                let image = GalleryImage::new(
                                                    photo.clone(),
                                                    photo_manager
                                                        .thumbnail_texture_for(photo, ui.ctx()),
                                                    selected_images.contains(&photo.path),
                                                );

                                                let image_response = ui.add(image);

                                                if image_response.clicked() {
                                                    let ctrl_held =
                                                        ui.input(|input| input.modifiers.ctrl);
                                                    if ctrl_held {
                                                        if selected_images.contains(&photo.path) {
                                                            selected_images.remove(&photo.path);
                                                        } else {
                                                            selected_images
                                                                .insert(photo.path.clone());
                                                        }
                                                    } else {
                                                        selected_images.clear();
                                                        selected_images.insert(photo.path.clone());
                                                    }
                                                }

                                                if image_response.double_clicked() {
                                                    response = Some(
                                                        ImageGalleryResponse::SelectPhotoPrimaryAction(
                                                            photo.clone(),
                                                        ),
                                                    );
                                                } else if image_response.secondary_clicked() {
                                                    response =
                                                        Some(ImageGalleryResponse::SelectPhotoSecondaryAction(
                                                            photo.clone(),
                                                        ));
                                                }
                                            });
                                        });
                                    }

                                    row.col(|ui| {
                                        ui.add(Spacer::new(spacer_width, row_height));
                                    });
                                }
                            });
                        })
                });
                
                ui.painter().rect_filled(
                    ui.available_rect_before_wrap(),
                    0.0,
                    Color32::from_gray(40),
                );

                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(20.0);
                    ui.add(
                        Image::from(Asset::larger())
                            .tint(Color32::WHITE)
                            .maintain_aspect_ratio(true)
                            .fit_to_exact_size(Vec2::splat(20.0)),
                    );
                    ui.add(Slider::new(&mut state.scale, 0.5..=1.5).show_value(true));
                    ui.add(
                        Image::from(Asset::smaller())
                            .tint(Color32::WHITE)
                            .maintain_aspect_ratio(true)
                            .fit_to_exact_size(Vec2::splat(20.0)),
                    );
                });
            });
        } else {
            ui.both_centered(|ui| ui.heading("Import photos or open a project to get started"));
        }

        response
    }
}

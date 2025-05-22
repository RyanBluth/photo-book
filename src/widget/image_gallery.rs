use std::{collections::HashSet, path::PathBuf};

use eframe::{
    egui::{Key, Ui},
    epaint::Vec2,
};

use egui::{Color32, Image, Layout, Slider};
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

#[derive(Debug, Clone)]
pub struct ImageGalleryResponse {
    /// A photo that was double-clicked (primary action)
    pub primary_action_photo: Option<Photo>,
    /// A photo that was right-clicked (secondary action)
    pub secondary_action_photo: Option<Photo>,
    /// A photo that was newly selected this frame
    pub selected_photo: Option<Photo>,
    /// Flag indicating if the selection was cleared this frame (e.g., by Escape key)
    pub selection_cleared: bool,
}

impl<'a> ImageGallery<'a> {
    pub fn show(
        ui: &mut Ui,
        state: &'a mut ImageGalleryState,
        scroll_to_path: Option<&PathBuf>,
    ) -> ImageGalleryResponse {
        let photo_manager: Singleton<PhotoManager> = Dependency::get();
        let selected_images = &mut state.selected_images;

        // Initialize response with defaults
        let mut primary_action_photo: Option<Photo> = None;
        let mut secondary_action_photo: Option<Photo> = None;
        let mut selected_photo: Option<Photo> = None;
        let mut selection_cleared = false;

        let has_photos = photo_manager.with_lock(|photo_manager| !photo_manager.photos.is_empty());

        if has_photos {
            ui.vertical(|ui| {
                if ui.input(|input| input.key_down(Key::Escape)) {
                    selected_images.clear();
                    selection_cleared = true;
                }

                let spacing = 10.0;

                let bottom_bar_height = 50.0;

                let mut table_size = ui.available_size();
                table_size.y -= bottom_bar_height;
                table_size = table_size.max(Vec2::splat(0.0));

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
                        .with_lock(|photo_manager| photo_manager.grouped_photos().clone());

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

                    let scroll_to_row_index = if let Some(scroll_to_path) = scroll_to_path {
                        scroll_to_row_index(scroll_to_path, num_columns, &grouped_photos, &row_metadatas)
                    } else {
                        None
                    };

                    let mut builder = egui_extras::TableBuilder::new(ui)
                        .min_scrolled_height(table_size.y)
                        .auto_shrink(false)
                        .columns(Column::exact(column_width), num_columns)
                        .column(Column::exact(spacer_width));

                    if let Some(row) = scroll_to_row_index {
                        builder = builder.scroll_to_row(row, None);
                    }

                    builder.body(|body| {
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
                                                        selected_images.insert(photo.path.clone());
                                                        selected_photo = Some(photo.clone());
                                                    }
                                                } else {
                                                    let was_empty = selected_images.is_empty();
                                                    let was_already_selected =
                                                        selected_images.contains(&photo.path);
                                                    selected_images.clear();
                                                    selected_images.insert(photo.path.clone());
                                                    // Only report as newly selected if it wasn't the only selection before
                                                    if was_empty
                                                        || !was_already_selected
                                                        || selected_images.len() != 1
                                                    {
                                                        selected_photo = Some(photo.clone());
                                                    }
                                                }
                                            }

                                            if image_response.double_clicked() {
                                                primary_action_photo = Some(photo.clone());
                                            } else if image_response.secondary_clicked() {
                                                secondary_action_photo = Some(photo.clone());
                                            }
                                        });
                                    });
                                }

                                row.col(|ui| {
                                    ui.add(Spacer::new(spacer_width, row_height));
                                });
                            }
                        });
                    });
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
        }

        ImageGalleryResponse {
            primary_action_photo,
            secondary_action_photo,
            selected_photo,
            selection_cleared,
        }
    }
}

fn scroll_to_row_index(
    scroll_to_path: &PathBuf,
    num_columns: usize,
    grouped_photos: &indexmap::IndexMap<String, indexmap::IndexMap<PathBuf, Photo>>,
    row_metadatas: &[RowMetadata],
) -> Option<usize> {
    let mut scroll_to_row: Option<usize> = None;

    let mut section_with_photo: Option<(String, usize)> = None;

    // Find which section and row contains this path
    for (section, photos) in grouped_photos {
        // Need to look at the map's keys for the path, not the Photo objects
        if let Some(pos) = photos
            .keys()
            .position(|photo_path| photo_path == scroll_to_path)
        {
            let row_idx = pos / num_columns;
            section_with_photo = Some((section.clone(), row_idx));
            break;
        }
    }

    // If found, look up the corresponding row
    if let Some((section, row_idx)) = section_with_photo {
        for (idx, metadata) in row_metadatas.iter().enumerate() {
            if metadata.section == section
                && !metadata.is_title
                && metadata.row_index_in_section == row_idx
            {
                scroll_to_row = Some(idx);
                break;
            }
        }
    }

    scroll_to_row
}

struct RowMetadata {
    height: f32,
    is_title: bool,
    section: String,
    row_index_in_section: usize,
}

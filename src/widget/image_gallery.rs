use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use eframe::{egui::Key, epaint::Vec2};

use egui::{
    Align, FontId, Frame, Image, Layout, Margin, MenuBar, PopupCloseBehavior, Rect, Response,
    RichText, Slider, Ui, UiBuilder, containers::menu::MenuConfig,
};
use egui_extras::{Column, TableBuilder};
use indexmap::IndexMap;

use crate::{
    assets::Asset,
    dep, dep_mut,
    modal::{manager::ModalManager, new_album::NewAlbumModal},
    model::{album::Album, photo_grouping::PhotoGrouping},
    photo::Photo,
    photo_database::PhotoQuery,
    photo_manager::{self, PhotoManager},
    selection_manager::{SelectionManager, SelectionModifiers},
    theme::color,
};

use super::{gallery_image::GalleryImage, spacer::Spacer};

const BAR_INNER_PADDING: i8 = 8;
const BASE_COLUMN_SIZE: f32 = 256.0;

#[derive(Debug, Clone)]
pub struct ImageGalleryState {
    pub scale: f32,
    layout_cache: Option<GalleryLayoutCache>,
}

impl Default for ImageGalleryState {
    fn default() -> Self {
        Self {
            scale: 1.0,
            layout_cache: None,
        }
    }
}

pub struct ImageGallery<'a> {
    #[allow(dead_code)]
    state: &'a mut ImageGalleryState,
}

#[derive(Debug, Clone)]
pub struct ImageGalleryResponse {
    /// A photo that was double-clicked (primary action)
    pub primary_action_photo: Option<Photo>,
    /// A photo that was right-clicked (secondary action)
    pub secondary_action_photo: Option<Photo>,
}

impl<'a> ImageGallery<'a> {
    pub fn show(
        ui: &mut Ui,
        state: &'a mut ImageGalleryState,
        scroll_to_path: Option<&PathBuf>,
    ) -> ImageGalleryResponse {
        let mut selection_snapshot = dep!(SelectionManager, |selection_manager| {
            selection_manager.snapshot()
        });

        // Initialize response with defaults
        let mut primary_action_photo: Option<Photo> = None;
        let mut secondary_action_photo: Option<Photo> = None;

        let has_photos = dep!(PhotoManager, |photo_manager| {
            photo_manager.photo_database.photo_count() > 0
        });

        let query_result = dep_mut!(PhotoManager, |photo_manager| photo_manager.grouped_photos());
        let grouped_photos = &query_result.groups;
        let gallery_background_rect = ui.available_rect_before_wrap();
        ui.painter()
            .rect_filled(gallery_background_rect, 0.0, color::SURFACE_XX_DARK);

        if has_photos {
            ui.vertical(|ui| {
                if !ui.ctx().text_edit_focused()
                    && ui.input_mut(|input| input.consume_key(egui::Modifiers::NONE, Key::Escape))
                    && !selection_snapshot.selected_paths.is_empty()
                {
                    selection_snapshot = dep_mut!(SelectionManager, |selection_manager| {
                        selection_manager.clear_this_frame(ui)
                    });
                }

                let spacing = 24.0;

                let gallery_rect: Rect = ui.available_rect_before_wrap();
                let bar_height = ui.spacing().interact_size.y + 2.0 * BAR_INNER_PADDING as f32;
                let top_bar_height = bar_height;
                let bottom_bar_height = bar_height;

                let top_bar_rect = Rect::from_min_size(
                    gallery_rect.left_top(),
                    Vec2::new(gallery_rect.width(), top_bar_height),
                );
                let bottom_bar_rect = Rect::from_min_max(
                    egui::pos2(
                        gallery_rect.left(),
                        (gallery_rect.bottom() - bottom_bar_height).max(top_bar_rect.bottom()),
                    ),
                    gallery_rect.right_bottom(),
                );
                let table_rect = Rect::from_min_max(
                    egui::pos2(gallery_rect.left() + 16.0, top_bar_rect.bottom() + 16.0),
                    egui::pos2(gallery_rect.right() - 16.0, bottom_bar_rect.top() - 16.0),
                );

                add_child_ui(ui, top_bar_rect, "image_gallery_top_bar", |ui| {
                    add_filter_menu(ui);
                });

                let table_size = table_rect.size().max(Vec2::splat(0.0));

                add_child_ui(ui, table_rect, "image_gallery_table_area", |ui| {
                    ui.spacing_mut().item_spacing = Vec2::splat(spacing);

                    // TableBuilder reserves this space for its vertical scrollbar. Account for it
                    // when fitting a column so the thumbnail itself is never clipped in a narrow
                    // gallery pane.
                    let available_table_width =
                        (table_size.x - ui.spacing().scroll.allocated_width()).max(0.0);
                    let rendered_scale = scale_to_fit_column(state.scale, available_table_width);
                    let column_width = BASE_COLUMN_SIZE * rendered_scale;
                    let row_height = column_width;
                    let num_columns: usize = (available_table_width / (column_width + spacing))
                        .floor()
                        .max(1.0) as usize;

                    let spacer_width = (table_size.x
                        - ((column_width + ui.spacing().item_spacing.x) * num_columns as f32)
                        - 10.0
                        - ui.spacing().item_spacing.x)
                        .max(0.0);

                    let layout_key = GalleryLayoutKey {
                        query_result_id: query_result.id(),
                        num_columns,
                        row_height_bits: row_height.to_bits(),
                    };
                    if state.layout_cache.as_ref().map(|cache| cache.key) != Some(layout_key) {
                        let rows: Arc<[RowMetadata]> = {
                            grouped_photos
                                .iter()
                                .flat_map(|(title, group)| {
                                    let rows = group.len().div_ceil(num_columns);

                                    let mut metadatas: Vec<RowMetadata> = vec![RowMetadata {
                                        height: 16.0,
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
                                .collect::<Vec<_>>()
                                .into()
                        };
                        state.layout_cache = Some(GalleryLayoutCache {
                            key: layout_key,
                            rows,
                        });
                    }
                    let row_metadatas = Arc::clone(&state.layout_cache.as_ref().unwrap().rows);

                    let scroll_to_row_index = if let Some(scroll_to_path) = scroll_to_path {
                        scroll_to_row_index(
                            scroll_to_path,
                            num_columns,
                            grouped_photos,
                            row_metadatas.as_ref(),
                        )
                    } else {
                        None
                    };

                    let mut builder = TableBuilder::new(ui)
                        .id_salt("image_gallery_table")
                        .min_scrolled_height(table_size.y)
                        .auto_shrink(false)
                        .columns(Column::exact(column_width), num_columns)
                        .column(Column::exact(spacer_width));

                    if let Some(row) = scroll_to_row_index {
                        builder = builder.scroll_to_row(row, None);
                    }

                    builder.body(|body| {
                        body.heterogeneous_rows(
                            row_metadatas.iter().map(|row| row.height),
                            |mut row| {
                                let row_index = row.index();
                                let metadata = &row_metadatas[row_index];
                                let offest = metadata.row_index_in_section * num_columns;
                                let group = grouped_photos.get(&metadata.section).unwrap();

                                if metadata.is_title {
                                    row.col(|ui| {
                                        ui.vertical(|ui| {
                                            ui.label(
                                                RichText::new(metadata.section.clone())
                                                    .font(FontId::proportional(16.0)),
                                            );
                                        });
                                    });
                                } else {
                                    for i in 0..num_columns {
                                        if offest + i >= group.len() {
                                            break;
                                        }

                                        row.col(|ui: &mut Ui| {
                                            let photo = &group[offest + i];
                                            let image = GalleryImage::new(
                                                photo.clone(),
                                                selection_snapshot
                                                    .selected_paths
                                                    .contains(&photo.path),
                                            );
                                            let image_response = ui.add(image);

                                            Self::image_context_menu(photo, &image_response);

                                            if image_response.clicked() {
                                                let ordered_photo_paths = query_result
                                                    .ordered_paths()
                                                    .cloned()
                                                    .collect::<Vec<_>>();
                                                let modifiers =
                                                    ui.input(|input| SelectionModifiers {
                                                        ctrl: input.modifiers.ctrl,
                                                        shift: input.modifiers.shift,
                                                    });
                                                selection_snapshot = dep_mut!(
                                                    SelectionManager,
                                                    |selection_manager| {
                                                        selection_manager.select_path(
                                                            ui,
                                                            &ordered_photo_paths,
                                                            &photo.path,
                                                            modifiers,
                                                        )
                                                    }
                                                );
                                            }

                                            if image_response.hovered() {
                                                dep_mut!(PhotoManager, |photo_manager| {
                                                    let _ = photo_manager
                                                        .preload_texture(photo, ui.ctx());
                                                });
                                            }

                                            if image_response.double_clicked() {
                                                primary_action_photo = Some(photo.clone());
                                            } else if image_response.secondary_clicked() {
                                                secondary_action_photo = Some(photo.clone());
                                            }
                                        });
                                    }

                                    row.col(|ui| {
                                        ui.add(Spacer::new(spacer_width, row_height));
                                    });
                                }
                            },
                        );
                    });
                });
                add_child_ui(ui, bottom_bar_rect, "image_gallery_bottom_bar", |ui| {
                    add_scale_controls(ui, &mut state.scale);
                });

                ui.advance_cursor_after_rect(gallery_rect);
            });
        } else {
            ui.advance_cursor_after_rect(gallery_background_rect);
        }

        ImageGalleryResponse {
            primary_action_photo,
            secondary_action_photo,
        }
    }

    fn image_context_menu(photo: &Photo, image_response: &Response) {
        let (photo_albums, not_belongs_to_albums) = dep!(PhotoManager, |photo_manager| {
            let mut photo_albums = Vec::new();
            let mut not_belongs_to_albums = Vec::new();

            for album in photo_manager.albums_iter().cloned() {
                if album.photos.contains(&photo.path) {
                    photo_albums.push(album);
                } else {
                    not_belongs_to_albums.push(album);
                }
            }

            (photo_albums, not_belongs_to_albums)
        });

        let _ = image_response.context_menu(|ui| {
            ui.menu_button("Add to album", |ui| {
                if ui.button("New Album").clicked() {
                    ModalManager::push(NewAlbumModal::with_photos(vec![photo.path.clone()]));
                }

                if !not_belongs_to_albums.is_empty() {
                    ui.separator();

                    for album in &not_belongs_to_albums {
                        if ui.button(&album.name).clicked() {
                            dep_mut!(PhotoManager, |photo_manager| {
                                photo_manager.add_to_album(&album.id, &photo.path);
                            });
                        }
                    }
                }
            });
            if !photo_albums.is_empty() {
                ui.menu_button("Remove from album", |ui| {
                    if photo_albums.is_empty() {
                        ui.label("No albums");
                    } else {
                        for album in &photo_albums {
                            if ui.button(&album.name).clicked() {
                                dep_mut!(PhotoManager, |photo_manager| {
                                    photo_manager.remove_from_album(&album.id, &photo.path);
                                });
                            }
                        }
                    }
                });
            }
        });
    }
}

fn add_child_ui(
    ui: &mut Ui,
    rect: Rect,
    id_salt: &'static str,
    add_contents: impl FnOnce(&mut Ui),
) {
    let mut child_ui = ui.new_child(
        UiBuilder::new()
            .id_salt(id_salt)
            .max_rect(rect)
            .layout(*ui.layout()),
    );
    add_contents(&mut child_ui);
}

fn add_filter_menu(ui: &mut Ui) {
    let get_current_filter = || dep!(PhotoManager, |pm| pm.get_current_filter().clone());

    ui.painter().rect_filled(
        ui.available_rect_before_wrap(),
        0.0,
        color::TOOLBAR_BACKGROUND,
    );

    Frame::NONE
        .inner_margin(Margin::same(BAR_INNER_PADDING))
        .show(ui, |ui| {
            MenuBar::new()
                .config(MenuConfig::new().close_behavior(PopupCloseBehavior::CloseOnClickOutside))
                .ui(ui, |ui| {
                    ui.menu_button("Rating", |ui| {
                        let mut new_filter = get_current_filter();

                        for rating in [None, Some(1), Some(2), Some(3)] {
                            let label = match rating {
                                None => "Unrated".to_string(),
                                Some(1) => "1 Star".to_string(),
                                Some(n) => format!("{} Stars", n),
                            };
                            let mut is_enabled = new_filter
                                .ratings
                                .as_ref()
                                .map(|ratings| ratings.contains(&rating))
                                .unwrap_or(false);

                            if ui.checkbox(&mut is_enabled, label).changed() {
                                let ratings = new_filter.ratings.get_or_insert_with(Vec::new);
                                if is_enabled {
                                    if !ratings.contains(&rating) {
                                        ratings.push(rating);
                                    }
                                } else {
                                    ratings.retain(|r| r != &rating);
                                }

                                if ratings.is_empty() {
                                    new_filter.ratings = None;
                                }
                            }
                        }

                        if get_current_filter() != new_filter {
                            dep_mut!(PhotoManager, |pm| pm.set_current_filter(new_filter));
                        }
                    });

                    ui.menu_button("Tags", |ui| {
                        let mut new_filter = get_current_filter();
                        let available_tags = dep!(PhotoManager, |pm| pm.all_tags());

                        if available_tags.is_empty() {
                            ui.label("No Tags");
                        } else {
                            for tag in available_tags {
                                let mut is_enabled = new_filter
                                    .tags
                                    .as_ref()
                                    .map(|tags| tags.contains(&tag))
                                    .unwrap_or(false);

                                if ui.checkbox(&mut is_enabled, &tag).changed() {
                                    let tags = new_filter.tags.get_or_insert_with(Vec::new);
                                    if is_enabled {
                                        if !tags.contains(&tag) {
                                            tags.push(tag.clone());
                                        }
                                    } else {
                                        tags.retain(|t| t != &tag);
                                    }

                                    if tags.is_empty() {
                                        new_filter.tags = None;
                                    }
                                }
                            }
                        }

                        if get_current_filter() != new_filter {
                            dep_mut!(PhotoManager, |pm| pm.set_current_filter(new_filter));
                        }
                    });

                    ui.menu_button("Albums", |ui| {
                        let mut new_filter = get_current_filter();
                        let mut available_albums = dep!(PhotoManager, |pm| {
                            pm.albums_iter()
                                .map(|album| (album.id.clone(), album.name.clone()))
                                .collect::<Vec<_>>()
                        });
                        available_albums.sort_by(|(_, left), (_, right)| {
                            left.to_lowercase().cmp(&right.to_lowercase())
                        });

                        if ui.radio(new_filter.album.is_none(), "All Albums").clicked() {
                            new_filter.album = None;
                        }

                        if available_albums.is_empty() {
                            ui.label("No Albums");
                        } else {
                            ui.separator();
                            for (album_id, album_name) in available_albums {
                                let is_selected = new_filter.album.as_ref() == Some(&album_id);
                                if ui.radio(is_selected, &album_name).clicked() {
                                    new_filter.album = Some(album_id);
                                }
                            }
                        }

                        if get_current_filter() != new_filter {
                            dep_mut!(PhotoManager, |pm| pm.set_current_filter(new_filter));
                        }
                    });

                    ui.menu_button("Grouping", |ui| {
                        let new_grouping =
                            dep!(PhotoManager, |pm| pm.get_current_filter().grouping);

                        if ui
                            .radio(new_grouping == PhotoGrouping::Date, "Date")
                            .clicked()
                        {
                            let mut filter =
                                dep!(PhotoManager, |pm| pm.get_current_filter().clone());
                            filter.grouping = PhotoGrouping::Date;
                            dep_mut!(PhotoManager, |pm| pm.set_current_filter(filter));
                        }

                        if ui
                            .radio(new_grouping == PhotoGrouping::Rating, "Rating")
                            .clicked()
                        {
                            let mut filter =
                                dep!(PhotoManager, |pm| pm.get_current_filter().clone());
                            filter.grouping = PhotoGrouping::Rating;
                            dep_mut!(PhotoManager, |pm| pm.set_current_filter(filter));
                        }

                        if ui
                            .radio(new_grouping == PhotoGrouping::Tag, "Tag")
                            .clicked()
                        {
                            let mut filter =
                                dep!(PhotoManager, |pm| pm.get_current_filter().clone());
                            filter.grouping = PhotoGrouping::Tag;
                            dep_mut!(PhotoManager, |pm| pm.set_current_filter(filter));
                        }
                    });

                    if ui.button("Clear All Filters").clicked() {
                        dep_mut!(PhotoManager, |pm| pm
                            .set_current_filter(PhotoQuery::default()));
                    }
                });
        });
}

fn add_scale_controls(ui: &mut Ui, scale: &mut f32) {
    ui.painter()
        .rect_filled(ui.available_rect_before_wrap(), 0.0, color::SURFACE_XX_DARK);

    Frame::NONE
        .inner_margin(Margin::same(BAR_INNER_PADDING))
        .show(ui, |ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add(
                    Image::from(Asset::larger())
                        .tint(color::ICON)
                        .maintain_aspect_ratio(true)
                        .fit_to_exact_size(Vec2::splat(20.0)),
                );
                ui.add(Slider::new(scale, 0.5..=1.5).show_value(true));
                ui.add(
                    Image::from(Asset::smaller())
                        .tint(color::ICON)
                        .maintain_aspect_ratio(true)
                        .fit_to_exact_size(Vec2::splat(20.0)),
                );
            });
        });
}

fn scale_to_fit_column(preferred_scale: f32, available_width: f32) -> f32 {
    preferred_scale.min(available_width / BASE_COLUMN_SIZE)
}

fn scroll_to_row_index(
    scroll_to_path: &PathBuf,
    num_columns: usize,
    grouped_photos: &IndexMap<String, IndexMap<PathBuf, Photo>>,
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

#[derive(Debug, Clone)]
struct RowMetadata {
    height: f32,
    is_title: bool,
    section: String,
    row_index_in_section: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GalleryLayoutKey {
    query_result_id: usize,
    num_columns: usize,
    row_height_bits: u32,
}

#[derive(Debug, Clone)]
struct GalleryLayoutCache {
    key: GalleryLayoutKey,
    rows: Arc<[RowMetadata]>,
}

#[cfg(test)]
mod tests {
    use super::{BASE_COLUMN_SIZE, scale_to_fit_column};

    #[test]
    fn keeps_preferred_scale_when_a_column_fits() {
        assert_eq!(scale_to_fit_column(1.0, BASE_COLUMN_SIZE), 1.0);
        assert_eq!(scale_to_fit_column(0.5, BASE_COLUMN_SIZE), 0.5);
    }

    #[test]
    fn reduces_scale_to_fit_a_single_column() {
        assert_eq!(scale_to_fit_column(1.0, 192.0), 0.75);
        assert_eq!(scale_to_fit_column(0.5, 64.0), 0.25);
    }

    #[test]
    fn caps_enlarged_columns_at_the_available_width() {
        assert_eq!(scale_to_fit_column(1.5, 320.0), 1.25);
    }
}

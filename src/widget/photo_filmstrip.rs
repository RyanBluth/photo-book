use std::{ops::Range, path::PathBuf};

use egui::{
    load::SizedTexture, style::ScrollAnimation, Align, CursorIcon, Image, Pos2, Rect, Response,
    ScrollArea, Sense, Spinner, Stroke, StrokeKind, Ui, UiBuilder, Vec2,
};

use crate::{
    cursor_manager::CursorManager,
    dep_mut,
    photo::{self, Photo},
    photo_manager::PhotoManager,
    theme::color,
};

const DEFAULT_BAR_HEIGHT: f32 = 120.0;
const MIN_BAR_HEIGHT: f32 = 92.0;
const MAX_BAR_HEIGHT: f32 = 420.0;
const RESIZE_HANDLE_HEIGHT: f32 = 8.0;
const RESIZE_LINE_HEIGHT: f32 = 2.0;
const CELL_ASPECT_RATIO: f32 = 184.0 / 148.0;
const CELL_SPACING: f32 = 12.0;
const CELL_PADDING: f32 = 12.0;
const VIRTUALIZED_OVERSCAN_CELLS: usize = 2;

#[derive(Debug, Clone)]
pub struct PhotoFilmstripState {
    centered_photo_path: Option<PathBuf>,
    height: f32,
}

impl Default for PhotoFilmstripState {
    fn default() -> Self {
        Self {
            centered_photo_path: None,
            height: DEFAULT_BAR_HEIGHT,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PhotoFilmstripResponse {
    pub selected_photo: Option<Photo>,
}

pub struct PhotoFilmstrip<'a> {
    current_photo: &'a Photo,
    state: &'a mut PhotoFilmstripState,
}

impl<'a> PhotoFilmstrip<'a> {
    pub fn new(current_photo: &'a Photo, state: &'a mut PhotoFilmstripState) -> Self {
        Self {
            current_photo,
            state,
        }
    }

    pub fn show(mut self, ui: &mut Ui) -> PhotoFilmstripResponse {
        let mut selected_photo = None;
        let mut photos = dep_mut!(PhotoManager, |photo_manager| {
            photo_manager
                .grouped_photos()
                .values()
                .flat_map(|group| group.values().cloned())
                .collect::<Vec<_>>()
        });

        if !photos
            .iter()
            .any(|photo| photo.path == self.current_photo.path)
        {
            photos.insert(0, self.current_photo.clone());
        }

        self.state.height = self.state.height.clamp(MIN_BAR_HEIGHT, MAX_BAR_HEIGHT);

        let desired_size = Vec2::new(ui.available_width(), self.state.height);
        let (bar_rect, _response) = ui.allocate_exact_size(desired_size, Sense::hover());
        let handle_rect = Rect::from_min_max(
            Pos2::new(bar_rect.left(), bar_rect.bottom() - RESIZE_HANDLE_HEIGHT),
            bar_rect.right_bottom(),
        );
        let content_rect: Rect = Rect::from_min_max(bar_rect.min, handle_rect.right_top())
            .shrink2(Vec2::new(0.0, CELL_SPACING));
        let cell_height = content_rect.height().max(1.0);
        let cell_width = cell_height * CELL_ASPECT_RATIO;

        ui.painter().rect_filled(bar_rect, 0.0, color::SURFACE_DARK);

        let row_width = thumbnail_row_width(photos.len(), cell_width);
        let edge_spacing = ((content_rect.width() - row_width) * 0.5).max(CELL_SPACING);
        ui.scope_builder(UiBuilder::new().max_rect(content_rect), |ui| {
            ui.set_clip_rect(content_rect);
            ui.spacing_mut().item_spacing = Vec2::new(CELL_SPACING, 0.0);

            ScrollArea::horizontal()
                .id_salt("viewer_photo_filmstrip")
                .max_height(content_rect.height())
                .auto_shrink([false, false])
                .show_viewport(ui, |ui, viewport| {
                    let total_width = edge_spacing * 2.0 + row_width;
                    ui.set_min_size(Vec2::new(total_width, cell_height));

                    if let Some(current_index) = photos
                        .iter()
                        .position(|photo| photo.path == self.current_photo.path)
                    {
                        let current_rect =
                            cell_rect(ui, current_index, edge_spacing, cell_width, cell_height);
                        if self.state.centered_photo_path.as_ref() != Some(&self.current_photo.path)
                        {
                            ui.ctx().global_style_mut(|style| {
                                style.scroll_animation = ScrollAnimation::none();
                            });
                            ui.scroll_to_rect(current_rect, Some(Align::Center));
                            self.state.centered_photo_path = Some(self.current_photo.path.clone());
                        }
                    }

                    for index in
                        visible_photo_range(viewport, photos.len(), edge_spacing, cell_width)
                    {
                        let photo = &photos[index];
                        let rect = cell_rect(ui, index, edge_spacing, cell_width, cell_height);
                        let response = ui
                            .push_id(("viewer_photo_filmstrip_cell", &photo.path), |ui| {
                                ui.scope_builder(UiBuilder::new().max_rect(rect), |ui| {
                                    thumbnail_cell(
                                        ui,
                                        photo,
                                        photo.path == self.current_photo.path,
                                        cell_width,
                                        cell_height,
                                    )
                                })
                                .inner
                            })
                            .inner;

                        if response.hovered() {
                            dep_mut!(PhotoManager, |photo_manager| {
                                let _ = photo_manager.preload_texture(photo, ui.ctx());
                            });
                        }

                        if response.clicked() {
                            selected_photo = Some(photo.clone());
                        }
                    }
                });
        });

        self.show_resize_handle(ui, handle_rect);

        PhotoFilmstripResponse { selected_photo }
    }

    fn show_resize_handle(&mut self, ui: &mut Ui, rect: Rect) {
        let response = ui.allocate_rect(rect, Sense::click_and_drag());
        let line_rect =
            Rect::from_center_size(rect.center(), Vec2::new(rect.width(), RESIZE_LINE_HEIGHT));
        let line_color = if response.hovered() || response.dragged() {
            color::SURFACE_EMPHASIS
        } else {
            color::SURFACE_MUTED
        };

        ui.painter().rect_filled(line_rect, 0.0, line_color);

        if response.hovered() || response.dragged() {
            dep_mut!(CursorManager, |cursor_manager| {
                cursor_manager.set_cursor(CursorIcon::ResizeRow);
            });
        }

        if response.dragged() {
            self.state.height =
                (self.state.height + response.drag_delta().y).clamp(MIN_BAR_HEIGHT, MAX_BAR_HEIGHT);
        }
    }
}

fn thumbnail_row_width(photo_count: usize, cell_width: f32) -> f32 {
    if photo_count == 0 {
        return 0.0;
    }

    photo_count as f32 * cell_width + photo_count.saturating_sub(1) as f32 * CELL_SPACING
}

fn visible_photo_range(
    viewport: Rect,
    photo_count: usize,
    edge_spacing: f32,
    cell_width: f32,
) -> Range<usize> {
    if photo_count == 0 {
        return 0..0;
    }

    let cell_stride = cell_width + CELL_SPACING;
    let first_visible = ((viewport.min.x - edge_spacing) / cell_stride).floor() as isize - 1;
    let last_visible = ((viewport.max.x - edge_spacing) / cell_stride).ceil() as isize + 1;

    let start = first_visible
        .saturating_sub(VIRTUALIZED_OVERSCAN_CELLS as isize)
        .clamp(0, photo_count as isize) as usize;
    let end = last_visible
        .saturating_add(VIRTUALIZED_OVERSCAN_CELLS as isize)
        .clamp(0, photo_count as isize) as usize;

    start..end.max(start)
}

fn cell_rect(ui: &Ui, index: usize, edge_spacing: f32, cell_width: f32, cell_height: f32) -> Rect {
    let cell_stride = cell_width + CELL_SPACING;
    let x = ui.max_rect().left() + edge_spacing + index as f32 * cell_stride;
    Rect::from_min_size(
        Pos2::new(x, ui.max_rect().top()),
        Vec2::new(cell_width, cell_height),
    )
}

fn thumbnail_cell(
    ui: &mut Ui,
    photo: &Photo,
    selected: bool,
    cell_width: f32,
    cell_height: f32,
) -> Response {
    let stroke_width = 3.0;
    let (rect, response) = ui.allocate_exact_size(
        Vec2::new(cell_width, cell_height - stroke_width * 2.0),
        Sense::click(),
    );
    let thumbnail = dep_mut!(PhotoManager, |photo_manager| {
        photo_manager.thumbnail_texture_for(photo, ui.ctx())
    });

    let photo_rect = rect
        .shrink2(Vec2::new(0.0, stroke_width * 2.0))
        .translate(Vec2::new(0.0, stroke_width));
    let thumbnail_rect = paint_thumbnail(ui, photo, photo_rect, thumbnail);

    if selected {
        ui.painter().rect_stroke(
            thumbnail_rect,
            stroke_width,
            Stroke::new(stroke_width, color::ACCENT),
            StrokeKind::Outside,
        );
    } else {
        ui.painter().rect_stroke(
            thumbnail_rect,
            stroke_width,
            Stroke::new(stroke_width, color::SURFACE_DARK),
            StrokeKind::Outside,
        );
    }

    response.on_hover_text(photo.path.display().to_string())
}

fn paint_thumbnail(
    ui: &mut Ui,
    photo: &Photo,
    bounds: Rect,
    thumbnail: anyhow::Result<Option<SizedTexture>>,
) -> Rect {
    let image_size = fitted_unrotated_image_size(photo, bounds.size());
    let image_rect = Rect::from_center_size(bounds.center(), image_size);
    let placeholder_rect =
        Rect::from_center_size(bounds.center(), rotated_bounding_size(photo, image_size));

    match thumbnail {
        Ok(Some(texture)) => {
            Image::from_texture(texture)
                .rotate(photo.metadata.rotation().radians(), Vec2::splat(0.5))
                .paint_at(ui, image_rect);
        }
        Ok(None) => {
            ui.painter()
                .rect_filled(placeholder_rect, 2.0, color::SURFACE_MUTED);
            ui.put(placeholder_rect, Spinner::new());
            ui.ctx().request_repaint();
        }
        Err(_) => {
            ui.painter()
                .rect_filled(placeholder_rect, 2.0, color::ERROR);
        }
    }

    placeholder_rect
}

fn fitted_unrotated_image_size(photo: &Photo, available_size: Vec2) -> Vec2 {
    let original_size = Vec2::new(
        photo.metadata.width() as f32,
        photo.metadata.height() as f32,
    );
    if original_size.x <= 0.0 || original_size.y <= 0.0 {
        return available_size;
    }

    let rotated_size = rotated_bounding_size(photo, original_size);
    let scale = (available_size.x / rotated_size.x).min(available_size.y / rotated_size.y);

    original_size * scale
}

fn rotated_bounding_size(photo: &Photo, size: Vec2) -> Vec2 {
    if photo.metadata.does_rotation_alter_dimensions() {
        Vec2::new(size.y, size.x)
    } else {
        size
    }
}

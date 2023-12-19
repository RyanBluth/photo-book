use eframe::{
    egui::{
        self, include_image, load::SizedTexture, Align, Button, CursorIcon, Image, InnerResponse,
        LayerId, Layout, Response, Sense, Ui, Widget,
    },
    emath::Rot2,
    epaint::{Color32, Mesh, Pos2, Rect, Shape, Stroke, Vec2},
};
use env_logger::fmt::Color;
use log::error;

use crate::{
    assets::Asset,
    dependencies::{Dependency, Singleton, SingletonFor},
    photo::Photo,
    photo_manager::{PhotoLoadResult, PhotoManager},
    utils::{RectExt, Truncate},
    widget::placeholder::RectPlaceholder,
};

pub enum CanvasResponse {
    Exit,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CanvasPhoto {
    pub id: usize,
    pub photo: Photo,
    pub transform_state: TransformableState,
    set_initial_position: bool,
}

impl CanvasPhoto {
    pub fn new(photo: Photo, id: usize) -> Self {
        let initial_rect = match photo.max_dimension() {
            crate::photo::MaxPhotoDimension::Width => {
                Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 1000.0 / photo.aspect_ratio()))
            }
            crate::photo::MaxPhotoDimension::Height => {
                Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0 * photo.aspect_ratio(), 1000.0))
            }
        };

        Self {
            photo,
            transform_state: TransformableState {
                rect: initial_rect,
                active_handle: None,
                is_moving: false,
                handle_mode: TransformHandleMode::Resize,
                rotation: 0.0,
            },
            id,
            set_initial_position: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct CanvasState {
    photos: Vec<CanvasPhoto>,
    active_photo: Option<usize>,
    zoom: f32,
    offset: Vec2,
}

impl CanvasState {
    pub fn new() -> Self {
        Self {
            photos: Vec::new(),
            active_photo: None,
            zoom: 1.0,
            offset: Vec2::ZERO,
        }
    }

    pub fn with_photo(photo: Photo) -> Self {
        let initial_rect = match photo.max_dimension() {
            crate::photo::MaxPhotoDimension::Width => {
                Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0, 1000.0 / photo.aspect_ratio()))
            }
            crate::photo::MaxPhotoDimension::Height => {
                Rect::from_min_size(Pos2::ZERO, Vec2::new(1000.0 * photo.aspect_ratio(), 1000.0))
            }
        };

        Self {
            photos: vec![CanvasPhoto {
                photo,
                transform_state: TransformableState {
                    rect: initial_rect,
                    active_handle: None,
                    is_moving: false,
                    handle_mode: TransformHandleMode::Resize,
                    rotation: 0.0,
                },
                id: 0,
                set_initial_position: false,
            }],
            active_photo: None,
            zoom: 1.0,
            offset: Vec2::ZERO,
        }
    }

    pub fn add_photo(&mut self, photo: Photo) {
        self.photos.push(CanvasPhoto::new(photo, self.photos.len()));
    }
}

pub struct Canvas<'a> {
    pub state: &'a mut CanvasState,
    photo_manager: Singleton<PhotoManager>,
}

impl<'a> Canvas<'a> {
    pub fn new(state: &'a mut CanvasState) -> Self {
        Self {
            state,
            photo_manager: Dependency::get(),
        }
    }

    pub fn show(&mut self, ui: &mut Ui) -> Option<CanvasResponse> {
        let available_rect = ui.available_rect_before_wrap();
        let response = ui.allocate_rect(available_rect, Sense::hover());
        let rect = response.rect;

        ui.input(|input| {
            self.state.zoom += input.scroll_delta.y * 0.02;
            self.state.zoom = self.state.zoom.max(0.1);
        });

        ui.input(|input| {
            if input.key_down(egui::Key::Space) {
                self.state.offset += input.pointer.delta();
                true
            } else {
                false
            }
        });

        ui.painter().rect_filled(rect, 0.0, Color32::BLACK);

        {
            // TEMP: page placeholder

            let page_rect = Rect::from_center_size(rect.center(), Vec2::new(1000.0, 1500.0));
            let page_rect = page_rect.translate(self.state.offset);

            let expansion = ((page_rect.size() * self.state.zoom) - page_rect.size()) * 0.5;
            let page_rect = page_rect.expand2(expansion);

            ui.painter().rect_filled(page_rect, 0.0, Color32::WHITE);
        }

        // Reset the cursor icon so it can be set by the transform widgets
        ui.ctx().set_cursor_icon(CursorIcon::Default);

        // Track if there was an active photo at the start of the frame.
        let has_active_photo_at_frame_start = self.state.active_photo.is_some();

        for canvas_photo in &mut self.state.photos.iter_mut() {
            // Move the photo to the center of the canvas if it hasn't been moved yet
            if !canvas_photo.set_initial_position {
                canvas_photo.transform_state.rect.set_center(rect.center());
                canvas_photo.set_initial_position = true;
            }

            ui.push_id(format!("CanvasPhoto_{}", canvas_photo.id), |ui| {
                self.photo_manager.with_lock_mut(|photo_manager| {
                    if let Ok(Some(texture)) =
                        photo_manager.texture_for(&canvas_photo.photo, &ui.ctx())
                    {
                        // If there is no active photo at the start of the frame then allow the widget to be enabled
                        // This allows photos on top of other photos to take priority since we draw back to front
                        let enabled = !has_active_photo_at_frame_start
                            || self
                                .state
                                .active_photo
                                .map(|x| x == canvas_photo.id)
                                .unwrap_or(false);

                        let mut transform_state = canvas_photo.transform_state.clone();

                        TransformableWidget::new(&mut transform_state).show(
                            ui,
                            available_rect,
                            enabled,
                            self.state.zoom,
                            self.state.offset,
                            |ui: &mut Ui, transformed_rect: Rect| {
                                let uv = Rect::from_min_max(
                                    Pos2::new(0.0, 0.0),
                                    Pos2 { x: 1.0, y: 1.0 },
                                );

                                let painter = ui.painter();
                                let mut mesh = Mesh::with_texture(texture.id);

                                // If the photo is rotated swap the width and height
                                let mesh_rect = if canvas_photo.photo.metadata.rotation().radians()
                                    == 0.0
                                    || canvas_photo.photo.metadata.rotation().radians()
                                        == std::f32::consts::PI
                                {
                                    transformed_rect
                                } else {
                                    Rect::from_center_size(
                                        transformed_rect.center(),
                                        Vec2::new(
                                            transformed_rect.height(),
                                            transformed_rect.width(),
                                        ),
                                    )
                                };

                                mesh.add_rect_with_uv(mesh_rect, uv, Color32::WHITE);

                                let mesh_center =
                                    mesh_rect.min + Vec2::splat(0.5) * mesh_rect.size();

                                mesh.rotate(
                                    Rot2::from_angle(
                                        canvas_photo.photo.metadata.rotation().radians(),
                                    ),
                                    mesh_center,
                                );
                                mesh.rotate(
                                    Rot2::from_angle(canvas_photo.transform_state.rotation),
                                    mesh_center,
                                );

                                painter.add(Shape::mesh(mesh));
                            },
                        );

                        if enabled
                            && (transform_state.is_moving
                                || transform_state.active_handle.is_some())
                        {
                            self.state.active_photo = Some(canvas_photo.id);
                        } else if self.state.active_photo == Some(canvas_photo.id) {
                            self.state.active_photo = None;
                        }

                        canvas_photo.transform_state = transform_state;
                    }
                })
            });
        }

        if ui.input(|input| input.key_pressed(egui::Key::Escape)) {
            return Some(CanvasResponse::Exit);
        }

        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum TransformHandle {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    MiddleTop,
    MiddleBottom,
    MiddleLeft,
    MiddleRight,
}

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum TransformHandleMode {
    Resize,
    Rotate,
}

impl TransformHandle {
    fn cursor(&self) -> CursorIcon {
        match self {
            TransformHandle::TopLeft => CursorIcon::ResizeNorthWest,
            TransformHandle::TopRight => CursorIcon::ResizeNorthEast,
            TransformHandle::BottomLeft => CursorIcon::ResizeSouthWest,
            TransformHandle::BottomRight => CursorIcon::ResizeSouthEast,
            TransformHandle::MiddleTop => CursorIcon::ResizeRow,
            TransformHandle::MiddleBottom => CursorIcon::ResizeRow,
            TransformHandle::MiddleLeft => CursorIcon::ResizeColumn,
            TransformHandle::MiddleRight => CursorIcon::ResizeColumn,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TransformableState {
    pub rect: Rect,
    pub active_handle: Option<TransformHandle>,
    pub is_moving: bool,
    pub handle_mode: TransformHandleMode,
    pub rotation: f32,
}

pub struct TransformableWidget<'a> {
    pub state: &'a mut TransformableState,
}

impl<'a> TransformableWidget<'a> {
    pub fn new(state: &'a mut TransformableState) -> Self {
        Self { state }
    }

    pub fn show<R>(
        &mut self,
        ui: &mut Ui,
        container_rect: Rect,
        enabled: bool,
        global_scale: f32,
        global_offset: Vec2,
        add_contents: impl FnOnce(&mut Ui, Rect) -> R,
    ) -> Response {
        let photo_center_relative_to_page = container_rect.center() - self.state.rect.center();

        // Scale the position of the center of the photo
        let scaled_photo_center = photo_center_relative_to_page * global_scale;

        // Translate photo to the new center position, adjusted for the global offset
        let translated_rect_center = container_rect.center() + global_offset - scaled_photo_center;

        // Scale the size of the photo
        let scaled_photo_size = self.state.rect.size() * global_scale;

        // Create the new scaled and translated rect for the photo
        let pre_rotated_inner_content_rect = Rect::from_min_size(
            translated_rect_center - scaled_photo_size / 2.0,
            scaled_photo_size,
        );

        let rotated_inner_content_rect =
            pre_rotated_inner_content_rect.rotate_bb_around_center(self.state.rotation);

        // Draw the mode selector above the inner content
        let mode_selector_response =
            self.draw_handle_mode_selector(ui, rotated_inner_content_rect.center_top());

        let response = ui.allocate_rect(
            rotated_inner_content_rect.union(mode_selector_response.rect),
            Sense::hover(),
        );

        let rect = response.rect;

        let handle_size = Vec2::splat(10.0); // Size of the resize handles

        let middle_point = |p1: Pos2, p2: Pos2| p1 + (p2 - p1) / 2.0;

        let handles = [
            (
                TransformHandle::TopLeft,
                rotated_inner_content_rect.left_top() - handle_size / 2.0,
            ),
            (
                TransformHandle::TopRight,
                rotated_inner_content_rect.right_top() - handle_size / 2.0,
            ),
            (
                TransformHandle::BottomLeft,
                rotated_inner_content_rect.left_bottom() - handle_size / 2.0,
            ),
            (
                TransformHandle::BottomRight,
                rotated_inner_content_rect.right_bottom() - handle_size / 2.0,
            ),
            (
                TransformHandle::MiddleTop,
                middle_point(
                    rotated_inner_content_rect.left_top(),
                    rotated_inner_content_rect.right_top(),
                ) - handle_size / 2.0,
            ),
            (
                TransformHandle::MiddleBottom,
                middle_point(
                    rotated_inner_content_rect.left_bottom(),
                    rotated_inner_content_rect.right_bottom(),
                ) - handle_size / 2.0,
            ),
            (
                TransformHandle::MiddleLeft,
                middle_point(
                    rotated_inner_content_rect.left_top(),
                    rotated_inner_content_rect.left_bottom(),
                ) - handle_size / 2.0,
            ),
            (
                TransformHandle::MiddleRight,
                middle_point(
                    rotated_inner_content_rect.right_top(),
                    rotated_inner_content_rect.right_bottom(),
                ) - handle_size / 2.0,
            ),
        ];

        // Interact with an expanded rect to include the handles which are partially outside the rect
        let interact_response = ui.interact(
            rotated_inner_content_rect.expand(handle_size.x / 2.0),
            response.id,
            Sense::click_and_drag(),
        );

        if enabled {
            for (handle, handle_pos) in &handles {
                let handle_rect = Rect::from_min_size(*handle_pos, handle_size);

                if !interact_response.is_pointer_button_down_on()
                    && self.state.active_handle == Some(*handle)
                {
                    self.state.active_handle = None;
                }

                if interact_response
                    .interact_pointer_pos()
                    .and_then(|pos| Some(handle_rect.contains(pos)))
                    .unwrap_or(false)
                    || self.state.active_handle == Some(*handle)
                {
                    let delta = interact_response.drag_delta() / global_scale;
                    match self.state.handle_mode {
                        TransformHandleMode::Resize => {
                            let mut new_rect = self.state.rect;

                            match handle {
                                TransformHandle::TopLeft => {
                                    new_rect.min.x += delta.x;
                                    new_rect.min.y += delta.y;
                                }
                                TransformHandle::TopRight => {
                                    new_rect.max.x += delta.x;
                                    new_rect.min.y += delta.y;
                                }
                                TransformHandle::BottomLeft => {
                                    new_rect.min.x += delta.x;
                                    new_rect.max.y += delta.y;
                                }
                                TransformHandle::BottomRight => {
                                    new_rect.max.x += delta.x;
                                    new_rect.max.y += delta.y;
                                }
                                TransformHandle::MiddleTop => {
                                    new_rect.min.y += delta.y;
                                }
                                TransformHandle::MiddleBottom => {
                                    new_rect.max.y += delta.y;
                                }
                                TransformHandle::MiddleLeft => {
                                    new_rect.min.x += delta.x;
                                }
                                TransformHandle::MiddleRight => {
                                    new_rect.max.x += delta.x;
                                }
                            };

                            self.state.active_handle = Some(*handle);
                            self.state.rect = new_rect;
                        }
                        TransformHandleMode::Rotate => {
                            if let Some(cursor_pos) = interact_response.interact_pointer_pos() {
                                let from_center_to_cursor = rect.center() - cursor_pos;
                                let from_center_to_cursor = from_center_to_cursor.normalized();

                                let from_center_to_cursor = from_center_to_cursor
                                    * (from_center_to_cursor.dot(Vec2::Y).acos()
                                        + std::f32::consts::PI / 2.0);

                                self.state.rotation =
                                    from_center_to_cursor.y.atan2(from_center_to_cursor.x);
                                self.state.active_handle = Some(*handle);
                            }
                        }
                    }
                }
            }

            if self.state.active_handle.is_none() {
                if interact_response.is_pointer_button_down_on()
                    && (interact_response
                        .interact_pointer_pos()
                        .and_then(|pos| Some(rect.contains(pos)))
                        .unwrap_or(false)
                        || self.state.is_moving)
                {
                    let delta = interact_response.drag_delta() / global_scale;
                    self.state.rect = self.state.rect.translate(delta);
                    self.state.is_moving = true;
                } else {
                    self.state.is_moving = false;
                }

                if interact_response.clicked() {
                    match self.state.handle_mode {
                        TransformHandleMode::Resize => {
                            self.state.handle_mode = TransformHandleMode::Rotate
                        }
                        TransformHandleMode::Rotate => {
                            self.state.handle_mode = TransformHandleMode::Resize
                        }
                    }
                }
            } else {
                self.state.is_moving = false;
            }
        } else {
            self.state.is_moving = false;
            self.state.active_handle = None;
        }

        let _inner_response = add_contents(ui, pre_rotated_inner_content_rect);

        ui.painter().rect_stroke(
            rotated_inner_content_rect,
            0.0,
            Stroke::new(
                3.0,
                if enabled {
                    Color32::GREEN
                } else {
                    Color32::RED
                },
            ),
        );

        // Draw the resize handles
        for (_, handle_pos) in &handles {
            let handle_rect = Rect::from_min_size(*handle_pos, handle_size);
            ui.painter().rect_filled(handle_rect, 0.0, Color32::WHITE);
        }

        ui.ctx().pointer_latest_pos().map(|pos| {
            for (handle, handle_pos) in &handles {
                let handle_rect = Rect::from_min_size(*handle_pos, handle_size);
                if handle_rect.contains(pos) {
                    match self.state.handle_mode {
                        TransformHandleMode::Resize => {
                            ui.ctx().set_cursor_icon(handle.cursor());
                        }
                        TransformHandleMode::Rotate => {
                            ui.ctx().set_cursor_icon(CursorIcon::Crosshair);
                        }
                    }
                    break;
                } else if rotated_inner_content_rect.contains(pos) {
                    ui.ctx().set_cursor_icon(CursorIcon::Move);
                }
            }
        });

        response
    }

    fn draw_handle_mode_selector(&mut self, ui: &mut Ui, bottom_center_origin: Pos2) -> Response {
        let width = 100.0;
        let height = 60.0;
        let margin_bottom = 20.0;
        let button_padding = 15.0;

        let button_size = Vec2::new(height - button_padding * 2.0, height - button_padding * 2.0);

        let response = ui.allocate_rect(
            Rect::from_points(&[
                bottom_center_origin + Vec2::new(0.0, -margin_bottom),
                bottom_center_origin + Vec2::new(-width * 0.5, -height),
                bottom_center_origin + Vec2::new(width * 0.5, -height),
            ]),
            Sense::hover(),
        );

        ui.painter()
            .rect(response.rect, 4.0, Color32::from_gray(40), Stroke::NONE);

        let left_half_rect =
            Rect::from_points(&[response.rect.left_top(), response.rect.center_bottom()]);

        let right_half_rect =
            Rect::from_points(&[response.rect.center_bottom(), response.rect.right_top()]);

        if ui
            .put(
                Rect::from_center_size(left_half_rect.center(), button_size),
                Button::image(
                    Image::from(Asset::resize())
                        .tint(Color32::WHITE)
                        .fit_to_exact_size(button_size * 0.8),
                )
                .fill(
                    if matches!(self.state.handle_mode, TransformHandleMode::Resize)
                        && !self.state.is_moving
                    {
                        Color32::from_gray(100)
                    } else {
                        Color32::from_gray(50)
                    },
                )
                .sense(Sense::click_and_drag()),
            )
            .clicked()
        {
            self.state.handle_mode = TransformHandleMode::Resize;
        }

        if ui
            .put(
                Rect::from_center_size(right_half_rect.center(), button_size),
                Button::image(
                    Image::from(Asset::rotate())
                        .tint(Color32::WHITE)
                        .fit_to_exact_size(button_size * 0.8),
                )
                .fill(
                    if matches!(self.state.handle_mode, TransformHandleMode::Rotate)
                        && !self.state.is_moving
                    {
                        Color32::from_gray(100)
                    } else {
                        Color32::from_gray(50)
                    },
                )
                .sense(Sense::click_and_drag()),
            )
            .clicked()
        {
            self.state.handle_mode = TransformHandleMode::Rotate;
        }

        response
    }
}

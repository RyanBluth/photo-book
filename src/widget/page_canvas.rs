use eframe::{
    egui::{
        self, load::SizedTexture, CursorIcon, Image, InnerResponse, LayerId, Layout, Response,
        Sense, Ui, Widget,
    },
    emath::Rot2,
    epaint::{Color32, Mesh, Pos2, Rect, Shape, Stroke, Vec2},
};
use log::error;

use crate::{
    dependencies::{Dependency, Singleton, SingletonFor},
    photo::Photo,
    photo_manager::{PhotoLoadResult, PhotoManager},
    utils::{ConstrainRect, Truncate},
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
            },
            id,
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
                },
                id: 0,
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

        ui.painter().rect_filled(rect, 0.0, Color32::BLACK);

        // Reset the cursor icon so it can be set by the transform widgets
        ui.ctx().set_cursor_icon(CursorIcon::Default);

        // Track if there was an active photo at the start of the frame.
        let has_active_photo_at_frame_start = self.state.active_photo.is_some();

        for canvas_photo in &mut self.state.photos.iter_mut() {
            self.photo_manager.with_lock_mut(|photo_manager| {
                if let Ok(Some(texture)) = photo_manager.texture_for(&canvas_photo.photo, &ui.ctx())
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
                        |ui: &mut Ui, transformed_rect: Rect| {
                            ui.with_layer_id(LayerId::background(), |ui| {
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
                                mesh.rotate(
                                    Rot2::from_angle(
                                        canvas_photo.photo.metadata.rotation().radians(),
                                    ),
                                    mesh_rect.min + Vec2::splat(0.5) * mesh_rect.size(),
                                );

                                painter.add(Shape::mesh(mesh));
                            });
                        },
                    );

                    if enabled
                        && (transform_state.is_moving || transform_state.active_handle.is_some())
                    {
                        self.state.active_photo = Some(canvas_photo.id);
                    } else if self.state.active_photo == Some(canvas_photo.id) {
                        self.state.active_photo = None;
                    }

                    canvas_photo.transform_state = transform_state;
                }
            })
        }

        if ui.input(|input| input.key_pressed(egui::Key::Escape)) {
            return Some(CanvasResponse::Exit);
        }

        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
enum TransformHandle {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
    MiddleTop,
    MiddleBottom,
    MiddleLeft,
    MiddleRight,
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
        add_contents: impl FnOnce(&mut Ui, Rect) -> R,
    ) -> Response {
        let response = ui.allocate_rect(self.state.rect, Sense::hover());

        let rect = response.rect;

        let handle_size = Vec2::splat(10.0); // Size of the resize handles

        let middle_point = |p1: Pos2, p2: Pos2| p1 + (p2 - p1) / 2.0;

        let handles = [
            (
                TransformHandle::TopLeft,
                rect.left_top() - handle_size / 2.0,
            ),
            (
                TransformHandle::TopRight,
                rect.right_top() - handle_size / 2.0,
            ),
            (
                TransformHandle::BottomLeft,
                rect.left_bottom() - handle_size / 2.0,
            ),
            (
                TransformHandle::BottomRight,
                rect.right_bottom() - handle_size / 2.0,
            ),
            (
                TransformHandle::MiddleTop,
                middle_point(rect.left_top(), rect.right_top()) - handle_size / 2.0,
            ),
            (
                TransformHandle::MiddleBottom,
                middle_point(rect.left_bottom(), rect.right_bottom()) - handle_size / 2.0,
            ),
            (
                TransformHandle::MiddleLeft,
                middle_point(rect.left_top(), rect.left_bottom()) - handle_size / 2.0,
            ),
            (
                TransformHandle::MiddleRight,
                middle_point(rect.right_top(), rect.right_bottom()) - handle_size / 2.0,
            ),
        ];

        if enabled {
            for (handle, handle_pos) in &handles {
                let handle_rect = Rect::from_min_size(*handle_pos, handle_size);
                let handle_response: Response =
                    ui.interact(handle_rect, ui.id(), Sense::click_and_drag());

                if !handle_response.is_pointer_button_down_on()
                    && self.state.active_handle == Some(*handle)
                {
                    self.state.active_handle = None;
                }

                if handle_response
                    .interact_pointer_pos()
                    .and_then(|pos| Some(handle_rect.contains(pos)))
                    .unwrap_or(false)
                    || self.state.active_handle == Some(*handle)
                {
                    let delta = handle_response.drag_delta();

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
            }

            if self.state.active_handle.is_none() {
                let move_response = ui.interact(rect, ui.id(), Sense::click_and_drag());

                if move_response.is_pointer_button_down_on() && (move_response
                    .interact_pointer_pos()
                    .and_then(|pos| Some(rect.contains(pos)))
                    .unwrap_or(false)
                    || self.state.is_moving)
                {
                    let delta = move_response.drag_delta();
                    self.state.rect = self.state.rect.translate(delta);
                    self.state.is_moving = true;
                } else {
                    self.state.is_moving = false;
                }
            } else {
                self.state.is_moving = false;
            }
        } else {
            self.state.is_moving = false;
            self.state.active_handle = None;
        }

        let _inner_response = add_contents(ui, rect);

        ui.painter().rect_stroke(
            rect,
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
                    ui.ctx().set_cursor_icon(handle.cursor());
                    break;
                } else if rect.contains(pos) {
                    ui.ctx().set_cursor_icon(CursorIcon::Move);
                }
            }
        });

        response
    }
}

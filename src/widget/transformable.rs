use eframe::{
    egui::{self, Button, CursorIcon, Image, Response, Sense, Ui},
    epaint::{Color32, Pos2, Rect, Stroke, Vec2},
};
use egui::{Id, LayerId, Order, StrokeKind, UiBuilder};

use crate::{
    assets::Asset,
    cursor_manager::CursorManager,
    dependencies::{Dependency, SingletonFor},
    utils::{IdExt, RectExt},
};

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
pub enum ResizeMode {
    Free,
    MirrorAxis,
    ConstrainedAspectRatio,
}

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
pub enum TransformHandleMode {
    Resize(ResizeMode),
    Rotate,
}

impl Default for TransformHandleMode {
    fn default() -> Self {
        Self::Resize(ResizeMode::Free)
    }
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
    pub last_frame_rotation: f32,
    pub change_in_rotation: Option<f32>,
    pub id: Id,
}

impl TransformableState {
    pub fn new(rect: Rect) -> Self {
        Self {
            rect,
            active_handle: None,
            is_moving: false,
            handle_mode: TransformHandleMode::Resize(ResizeMode::Free),
            rotation: 0.0,
            last_frame_rotation: 0.0,
            change_in_rotation: None,
            id: Id::random(),
        }
    }

    pub fn to_local_space(&self, parent: &TransformableState) -> Self {
        let mut new_rect = self.rect;
        new_rect.set_center(parent.rect.center() - self.rect.center().to_vec2());

        TransformableState {
            rect: new_rect,
            active_handle: self.active_handle,
            is_moving: self.is_moving,
            handle_mode: self.handle_mode,
            rotation: 0.0,
            last_frame_rotation: 0.0,
            change_in_rotation: None,
            id: self.id,
        }
    }
}

pub struct TransformableWidget<'a> {
    pub state: &'a mut TransformableState,
}

#[derive(Debug, Clone, PartialEq)]
pub enum _TransformableWidgetResponseAction {
    PushHistory,
}

#[derive(Debug, Clone)]
pub struct TransformableWidgetResponse<Inner> {
    pub _inner: Inner,
    pub _began_moving: bool,
    pub _began_resizing: bool,
    pub _began_rotating: bool,
    pub ended_moving: bool,
    pub ended_resizing: bool,
    pub ended_rotating: bool,
    pub mouse_down: bool,
    pub _clicked: bool,
    pub double_clicked: bool,
}

impl<'a> TransformableWidget<'a> {
    const HANDLE_SIZE: Vec2 = Vec2::splat(10.0);

    pub fn new(state: &'a mut TransformableState) -> Self {
        Self { state }
    }

    pub fn show<R>(
        &mut self,
        ui: &mut Ui,
        pre_scaled_container_rect: Rect,
        global_scale: f32,
        active: bool,
        rotatable: bool,
        add_contents: impl FnOnce(&mut Ui, Rect, &mut TransformableState) -> R,
    ) -> TransformableWidgetResponse<R> {
        let initial_is_moving = self.state.is_moving;
        let initial_active_handle = self.state.active_handle;
        let initial_mode = self.state.handle_mode;

        self.state.last_frame_rotation = self.state.rotation;

        // Translate photo to the new left_top position, adjusted for the global offset
        let translated_rect_left_top = pre_scaled_container_rect.left_top()
            + (self.state.rect.left_top() * global_scale).to_vec2();

        // Scale the size of the photo
        let scaled_photo_size = self.state.rect.size() * global_scale;

        // Create the new scaled and translated rect for the photo
        let pre_rotated_inner_content_rect: Rect =
            Rect::from_min_size(translated_rect_left_top, scaled_photo_size);

        let rotated_inner_content_rect =
            pre_rotated_inner_content_rect.rotate_bb_around_center(self.state.rotation);

        // Get the actual rotated corners of the pre-rotated rect
        // Order: [top_left, top_right, bottom_left, bottom_right]
        let rotated_corners = pre_rotated_inner_content_rect.rotated_corners(self.state.rotation);
        let mut response = if active {
            if rotatable {
                let mode_selector_response =
                    self.draw_handle_mode_selector(ui, rotated_inner_content_rect.center_top());

                ui.allocate_rect(rotated_inner_content_rect, Sense::click_and_drag())
                    .union(mode_selector_response)
            } else {
                ui.allocate_rect(rotated_inner_content_rect, Sense::click_and_drag())
            }
        } else {
            ui.allocate_rect(rotated_inner_content_rect, Sense::click_and_drag())
        };

        response.id = self.state.id;

        let rect = response.rect;

        let middle_point = |p1: Pos2, p2: Pos2| p1 + (p2 - p1) / 2.0;

        let handles = [
            (
                TransformHandle::TopLeft,
                rotated_corners[0] - Self::HANDLE_SIZE / 2.0,
            ),
            (
                TransformHandle::TopRight,
                rotated_corners[1] - Self::HANDLE_SIZE / 2.0,
            ),
            (
                TransformHandle::BottomLeft,
                rotated_corners[2] - Self::HANDLE_SIZE / 2.0,
            ),
            (
                TransformHandle::BottomRight,
                rotated_corners[3] - Self::HANDLE_SIZE / 2.0,
            ),
            (
                TransformHandle::MiddleTop,
                middle_point(rotated_corners[0], rotated_corners[1]) - Self::HANDLE_SIZE / 2.0,
            ),
            (
                TransformHandle::MiddleBottom,
                middle_point(rotated_corners[2], rotated_corners[3]) - Self::HANDLE_SIZE / 2.0,
            ),
            (
                TransformHandle::MiddleLeft,
                middle_point(rotated_corners[0], rotated_corners[2]) - Self::HANDLE_SIZE / 2.0,
            ),
            (
                TransformHandle::MiddleRight,
                middle_point(rotated_corners[1], rotated_corners[3]) - Self::HANDLE_SIZE / 2.0,
            ),
        ];

        // Interact with an expanded rect to include the handles which are partially outside the rect
        let interact_response: Response = ui.interact(
            rotated_inner_content_rect.expand(Self::HANDLE_SIZE.x / 2.0),
            response.id,
            Sense::click_and_drag(),
        );

        if active {
            for (handle, rotated_handle_pos) in &handles {
                let handle_rect: Rect = Rect::from_min_size(*rotated_handle_pos, Self::HANDLE_SIZE);
                if !interact_response.is_pointer_button_down_on()
                    && self.state.active_handle == Some(*handle)
                {
                    self.state.change_in_rotation = None;
                    self.state.active_handle = None;
                }

                if (interact_response
                    .interact_pointer_pos()
                    .map(|pos| handle_rect.contains(pos))
                    .unwrap_or(false)
                    && self.state.active_handle.is_none())
                    || self.state.active_handle == Some(*handle)
                {
                    let delta = interact_response.drag_delta() / global_scale;

                    let (shift_pressed, alt_pressed) = ui
                        .ctx()
                        .input(|input| (input.modifiers.shift, input.modifiers.alt));

                    match (self.state.handle_mode, shift_pressed, alt_pressed) {
                        (TransformHandleMode::Resize(ResizeMode::MirrorAxis), _, _)
                        | (TransformHandleMode::Resize(ResizeMode::Free), false, true) => {
                            let mut new_rect = self.state.rect;

                            match handle {
                                TransformHandle::TopLeft => {
                                    new_rect.min.x += delta.x;
                                    new_rect.min.y += delta.y;

                                    new_rect.max.x -= delta.x;
                                    new_rect.max.y -= delta.y;
                                }
                                TransformHandle::TopRight => {
                                    new_rect.max.x += delta.x;
                                    new_rect.min.y += delta.y;

                                    new_rect.min.x -= delta.x;
                                    new_rect.max.y -= delta.y;
                                }
                                TransformHandle::BottomLeft => {
                                    new_rect.min.x += delta.x;
                                    new_rect.max.y += delta.y;

                                    new_rect.max.x -= delta.x;
                                    new_rect.min.y -= delta.y;
                                }
                                TransformHandle::BottomRight => {
                                    new_rect.max.x += delta.x;
                                    new_rect.max.y += delta.y;

                                    new_rect.min.x -= delta.x;
                                    new_rect.min.y -= delta.y;
                                }
                                TransformHandle::MiddleTop => {
                                    new_rect.min.y += delta.y;
                                    new_rect.max.y -= delta.y;
                                }
                                TransformHandle::MiddleBottom => {
                                    new_rect.max.y += delta.y;
                                    new_rect.min.y -= delta.y;
                                }
                                TransformHandle::MiddleLeft => {
                                    new_rect.min.x += delta.x;
                                    new_rect.max.x -= delta.x;
                                }
                                TransformHandle::MiddleRight => {
                                    new_rect.max.x += delta.x;
                                    new_rect.min.x -= delta.x;
                                }
                            };

                            self.state.active_handle = Some(*handle);
                            self.state.rect = new_rect;
                        }
                        (TransformHandleMode::Resize(ResizeMode::ConstrainedAspectRatio), _, _)
                        | (TransformHandleMode::Resize(ResizeMode::Free), true, false) => {
                            let mut new_rect = self.state.rect;

                            let (ratio_x, ratio_y) = if new_rect.width() > new_rect.height() {
                                (new_rect.width() / new_rect.height(), 1.0)
                            } else {
                                (1.0, new_rect.height() / new_rect.width())
                            };

                            let max_delta = delta.x.min(delta.y);
                            let delta_x = max_delta * ratio_x;
                            let delta_y = max_delta * ratio_y;

                            match handle {
                                TransformHandle::TopLeft => {
                                    new_rect.min.x += delta_x;
                                    new_rect.min.y += delta_y;
                                }
                                TransformHandle::TopRight => {
                                    let max_delta = delta.x.abs().max(delta.y.abs());

                                    if delta.x.abs() > delta.y.abs() && delta.x > 0.0 {
                                        new_rect.max.x += max_delta * ratio_x;
                                        new_rect.min.y -= max_delta * ratio_y;
                                    } else if delta.x.abs() > delta.y.abs() && delta.x < 0.0 {
                                        new_rect.max.x -= max_delta * ratio_x;
                                        new_rect.min.y += max_delta * ratio_y;
                                    } else if delta.y.abs() > delta.x.abs() && delta.y > 0.0 {
                                        new_rect.max.x -= max_delta * ratio_x;
                                        new_rect.min.y += max_delta * ratio_y;
                                    } else if delta.y.abs() > delta.x.abs() && delta.y < 0.0 {
                                        new_rect.max.x += max_delta * ratio_x;
                                        new_rect.min.y -= max_delta * ratio_y;
                                    }
                                }
                                TransformHandle::BottomLeft => {
                                    let max_delta = delta.x.abs().max(delta.y.abs());

                                    if delta.x.abs() > delta.y.abs() && delta.x > 0.0 {
                                        new_rect.min.x += max_delta * ratio_x;
                                        new_rect.max.y -= max_delta * ratio_y;
                                    } else if delta.x.abs() > delta.y.abs() && delta.x < 0.0 {
                                        new_rect.min.x -= max_delta * ratio_x;
                                        new_rect.max.y += max_delta * ratio_y;
                                    } else if delta.y.abs() > delta.x.abs() && delta.y > 0.0 {
                                        new_rect.min.x -= max_delta * ratio_x;
                                        new_rect.max.y += max_delta * ratio_y;
                                    } else if delta.y.abs() > delta.x.abs() && delta.y < 0.0 {
                                        new_rect.min.x += max_delta * ratio_x;
                                        new_rect.max.y -= max_delta * ratio_y;
                                    }
                                }
                                TransformHandle::BottomRight => {
                                    new_rect.max.x += delta_x;
                                    new_rect.max.y += delta_y;
                                }
                                TransformHandle::MiddleTop => {
                                    new_rect.min.y -= delta.y * ratio_y * -1.0;
                                    new_rect.max.y += delta.y * ratio_y * -1.0;
                                    new_rect.min.x -= delta.y * ratio_x * -1.0;
                                    new_rect.max.x += delta.y * ratio_x * -1.0;
                                }
                                TransformHandle::MiddleLeft => {
                                    new_rect.min.y += delta.x * ratio_y;
                                    new_rect.max.y -= delta.x * ratio_y;
                                    new_rect.min.x += delta.x * ratio_x;
                                    new_rect.max.x -= delta.x * ratio_x;
                                }
                                TransformHandle::MiddleRight => {
                                    new_rect.min.y -= delta.x * ratio_y;
                                    new_rect.max.y += delta.x * ratio_y;
                                    new_rect.min.x -= delta.x * ratio_x;
                                    new_rect.max.x += delta.x * ratio_x;
                                }
                                TransformHandle::MiddleBottom => {
                                    new_rect.min.y -= delta.y * ratio_y;
                                    new_rect.max.y += delta.y * ratio_y;
                                    new_rect.min.x -= delta.y * ratio_x;
                                    new_rect.max.x += delta.y * ratio_x;
                                }
                            };

                            self.state.active_handle = Some(*handle);
                            self.state.rect = new_rect;
                        }
                        (TransformHandleMode::Resize(ResizeMode::Free), _, _) => {
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
                        (TransformHandleMode::Rotate, _, _) => {
                            if let Some(cursor_pos) = interact_response.interact_pointer_pos() {
                                // Use the pre-rotated rect center, which is stable during rotation
                                let center = pre_rotated_inner_content_rect.center();

                                // Calculate current cursor angle relative to center
                                let from_center_to_cursor = cursor_pos - center;
                                let current_cursor_angle =
                                    f32::atan2(from_center_to_cursor.y, from_center_to_cursor.x);

                                // If this is the first frame of rotation, store the offset
                                // change_in_rotation = initial_rotation - initial_cursor_angle
                                if self.state.change_in_rotation.is_none() {
                                    self.state.change_in_rotation =
                                        Some(self.state.rotation - current_cursor_angle);
                                }

                                self.state.rotation =
                                    self.state.change_in_rotation.unwrap() + current_cursor_angle;
                                self.state.active_handle = Some(*handle);
                            }
                        }
                    }
                }
            }

            if self.state.active_handle.is_none() {
                if interact_response.is_pointer_button_down_on()
                    && (self.state.is_moving
                        || interact_response
                            .interact_pointer_pos()
                            .map(|pos| rect.contains(pos))
                            .unwrap_or(false))
                {
                    let delta = interact_response.drag_delta() / global_scale;
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
            self.state.change_in_rotation = None;
        }

        let inner_response = add_contents(ui, pre_rotated_inner_content_rect, self.state);

        if active {
            self.draw_bounds_with_handles(ui, &rotated_corners, &handles);
            self.update_cursor(ui, &rotated_inner_content_rect, &handles);
        }

        TransformableWidgetResponse {
            _inner: inner_response,
            _began_moving: !initial_is_moving && self.state.is_moving,
            _began_resizing: initial_active_handle.is_none()
                && self.state.active_handle.is_some()
                && matches!(initial_mode, TransformHandleMode::Resize(_)),
            _began_rotating: initial_active_handle.is_none()
                && self.state.active_handle.is_some()
                && matches!(initial_mode, TransformHandleMode::Rotate),
            ended_moving: initial_is_moving && !self.state.is_moving,
            ended_resizing: initial_active_handle.is_some()
                && self.state.active_handle.is_none()
                && matches!(initial_mode, TransformHandleMode::Resize(_)),
            ended_rotating: initial_active_handle.is_some()
                && self.state.active_handle.is_none()
                && matches!(initial_mode, TransformHandleMode::Rotate),
            mouse_down: interact_response.is_pointer_button_down_on(),
            _clicked: interact_response.clicked(),
            double_clicked: interact_response.double_clicked(),
        }
    }

    fn update_cursor(
        &self,
        ui: &mut Ui,
        rotated_inner_content_rect: &Rect,
        handles: &[(TransformHandle, Pos2)],
    ) {
        ui.ctx().pointer_latest_pos().map(|pos| {
            for (handle, handle_pos) in handles {
                let handle_rect = Rect::from_min_size(*handle_pos, Self::HANDLE_SIZE);
                if handle_rect.contains(pos) {
                    match self.state.handle_mode {
                        TransformHandleMode::Resize(_) => {
                            Dependency::<CursorManager>::get().with_lock_mut(|cursor_manager| {
                                cursor_manager.set_cursor(handle.cursor());
                            });
                        }
                        TransformHandleMode::Rotate => {
                            Dependency::<CursorManager>::get().with_lock_mut(|cursor_manager| {
                                cursor_manager.set_cursor(CursorIcon::Crosshair);
                            });
                        }
                    }
                    break;
                } else if rotated_inner_content_rect.contains(pos) {
                    Dependency::<CursorManager>::get().with_lock_mut(|cursor_manager| {
                        cursor_manager.set_cursor(CursorIcon::Move);
                    });
                }
            }
        });
    }

    fn draw_bounds_with_handles(
        &self,
        ui: &mut Ui,
        rotated_corners: &[Pos2; 4],
        handles: &[(TransformHandle, Pos2)],
    ) {
        let painter = ui.painter();
        let stroke = Stroke::new(2.0, Color32::GRAY);

        painter.line_segment([rotated_corners[0], rotated_corners[1]], stroke); // top edge
        painter.line_segment([rotated_corners[1], rotated_corners[3]], stroke); // right edge
        painter.line_segment([rotated_corners[3], rotated_corners[2]], stroke); // bottom edge
        painter.line_segment([rotated_corners[2], rotated_corners[0]], stroke); // left edge

        // Draw the resize handles
        for (handle, handle_pos) in handles {
            let handle_rect = Rect::from_min_size(*handle_pos, Self::HANDLE_SIZE);
            painter.rect(
                handle_rect,
                1.0,
                if Some(handle) == self.state.active_handle.as_ref() {
                    Color32::RED
                } else {
                    Color32::WHITE
                },
                Stroke::new(2.0, Color32::BLACK),
                StrokeKind::Outside,
            );
        }
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

        ui.scope_builder(
            UiBuilder::new().layer_id(LayerId {
                order: Order::Tooltip,
                id: Id::new("transformable_mode_selector_layer"),
            }),
            |ui| {
                ui.painter().rect(
                    response.rect,
                    4.0,
                    Color32::from_gray(40),
                    Stroke::NONE,
                    StrokeKind::Outside,
                );

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
                            if matches!(self.state.handle_mode, TransformHandleMode::Resize(_)) {
                                Color32::from_gray(100)
                            } else {
                                Color32::from_gray(50)
                            },
                        )
                        .sense(Sense::click()),
                    )
                    .clicked()
                {
                    self.state.handle_mode = TransformHandleMode::Resize(ResizeMode::Free);
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
                            if matches!(self.state.handle_mode, TransformHandleMode::Rotate) {
                                Color32::from_gray(100)
                            } else {
                                Color32::from_gray(50)
                            },
                        )
                        .sense(Sense::click()),
                    )
                    .clicked()
                {
                    self.state.handle_mode = TransformHandleMode::Rotate;
                }

                response
            },
        )
        .inner
    }
}

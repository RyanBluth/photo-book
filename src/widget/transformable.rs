use eframe::{
    egui::{self, Button, CursorIcon, Image, Response, Sense, Ui},
    epaint::{Color32, Pos2, Rect, Stroke, Vec2},
};
use egui::{Id, StrokeKind};

use crate::{
    assets::Asset,
    cursor_manager::CursorManager,
    dependencies::{Dependency, SingletonFor},
    utils::RectExt,
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
pub enum TransformableWidgetResponseAction {
    PushHistory,
}

#[derive(Debug, Clone)]
pub struct TransformableWidgetResponse<Inner> {
    pub inner: Inner,
    pub began_moving: bool,
    pub began_resizing: bool,
    pub began_rotating: bool,
    pub ended_moving: bool,
    pub ended_resizing: bool,
    pub ended_rotating: bool,
    pub mouse_down: bool,
    pub clicked: bool,
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

        let mut response = if active {
            // Draw the mode selector above the inner content
            let mode_selector_response =
                self.draw_handle_mode_selector(ui, rotated_inner_content_rect.center_top());

            ui.allocate_rect(rotated_inner_content_rect, Sense::click_and_drag())
                .union(mode_selector_response)
        } else {
            ui.allocate_rect(rotated_inner_content_rect, Sense::click_and_drag())
        };

        response.id = self.state.id;

        let rect = response.rect;

        let middle_point = |p1: Pos2, p2: Pos2| p1 + (p2 - p1) / 2.0;

        let handles = [
            (
                TransformHandle::TopLeft,
                rotated_inner_content_rect.left_top() - Self::HANDLE_SIZE / 2.0,
            ),
            (
                TransformHandle::TopRight,
                rotated_inner_content_rect.right_top() - Self::HANDLE_SIZE / 2.0,
            ),
            (
                TransformHandle::BottomLeft,
                rotated_inner_content_rect.left_bottom() - Self::HANDLE_SIZE / 2.0,
            ),
            (
                TransformHandle::BottomRight,
                rotated_inner_content_rect.right_bottom() - Self::HANDLE_SIZE / 2.0,
            ),
            (
                TransformHandle::MiddleTop,
                middle_point(
                    rotated_inner_content_rect.left_top(),
                    rotated_inner_content_rect.right_top(),
                ) - Self::HANDLE_SIZE / 2.0,
            ),
            (
                TransformHandle::MiddleBottom,
                middle_point(
                    rotated_inner_content_rect.left_bottom(),
                    rotated_inner_content_rect.right_bottom(),
                ) - Self::HANDLE_SIZE / 2.0,
            ),
            (
                TransformHandle::MiddleLeft,
                middle_point(
                    rotated_inner_content_rect.left_top(),
                    rotated_inner_content_rect.left_bottom(),
                ) - Self::HANDLE_SIZE / 2.0,
            ),
            (
                TransformHandle::MiddleRight,
                middle_point(
                    rotated_inner_content_rect.right_top(),
                    rotated_inner_content_rect.right_bottom(),
                ) - Self::HANDLE_SIZE / 2.0,
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
                                let from_cursor_to_center =
                                    cursor_pos - rotated_inner_content_rect.center();

                                let from_rotated_handle_to_center =
                                    Rect::from_min_size(*rotated_handle_pos, Self::HANDLE_SIZE)
                                        .center()
                                        - rotated_inner_content_rect.center();

                                let rotated_signed_angle =
                                    f32::atan2(from_cursor_to_center.y, from_cursor_to_center.x)
                                        - f32::atan2(
                                            from_rotated_handle_to_center.y,
                                            from_rotated_handle_to_center.x,
                                        );

                                self.state.rotation += rotated_signed_angle
                                    - self.state.change_in_rotation.unwrap_or(0.0);
                                self.state.change_in_rotation = Some(rotated_signed_angle);

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

                if interact_response.double_clicked() {
                    match self.state.handle_mode {
                        TransformHandleMode::Resize(_) => {
                            self.state.handle_mode = TransformHandleMode::Rotate
                        }
                        TransformHandleMode::Rotate => {
                            self.state.handle_mode = TransformHandleMode::Resize(ResizeMode::Free)
                        }
                    }
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
            self.draw_bounds_with_handles(ui, &rotated_inner_content_rect, &handles);
            self.update_cursor(ui, &rotated_inner_content_rect, &handles);
        }

        TransformableWidgetResponse {
            inner: inner_response,
            began_moving: !initial_is_moving && self.state.is_moving,
            began_resizing: initial_active_handle.is_none()
                && self.state.active_handle.is_some()
                && matches!(initial_mode, TransformHandleMode::Resize(_)),
            began_rotating: initial_active_handle.is_none()
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
            clicked: interact_response.clicked(),
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
        rotated_content_rect: &Rect,
        handles: &[(TransformHandle, Pos2)],
    ) {
        ui.painter()
            .rect_stroke(*rotated_content_rect, 0.0, Stroke::new(2.0, Color32::GRAY), StrokeKind::Outside);

        // Draw the resize handles
        for (handle, handle_pos) in handles {
            let handle_rect = Rect::from_min_size(*handle_pos, Self::HANDLE_SIZE);
            ui.painter().rect(
                handle_rect,
                1.0,
                if Some(handle) == self.state.active_handle.as_ref() {
                    Color32::RED
                } else {
                    Color32::WHITE
                },
                Stroke::new(2.0, Color32::BLACK),
                StrokeKind::Outside
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

        ui.painter()
            .rect(response.rect, 4.0, Color32::from_gray(40), Stroke::NONE, StrokeKind::Outside);

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
    }
}

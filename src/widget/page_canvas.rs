use eframe::{
    egui::{
        self, load::SizedTexture, CursorIcon, Image, InnerResponse, Layout, Response, Sense, Ui,
        Widget,
    },
    emath::Rot2,
    epaint::{Color32, Mesh, Pos2, Rect, Shape, Stroke, Vec2},
};
use log::error;

use crate::{
    dependencies::{Dependency, Singleton, SingletonFor},
    photo::Photo,
    photo_manager::{PhotoLoadResult, PhotoManager},
    utils::Truncate,
    widget::placeholder::RectPlaceholder,
};

#[derive(Debug, Clone, PartialEq)]
pub struct CanvasPhoto {
    pub photo: Photo,
    pub transform_state: TransformableState,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CanvasState {
    pub photos: Vec<CanvasPhoto>,
}

impl CanvasState {
    pub fn new() -> Self {
        Self { photos: Vec::new() }
    }

    pub fn with_photo(photo: Photo) -> Self {
        Self {
            photos: vec![CanvasPhoto {
                photo,
                transform_state: TransformableState {
                    rect: Rect::from_min_size(Pos2::ZERO, Vec2::new(256.0, 256.0)),
                    active_handle: None,
                },
            }],
        }
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

    pub fn show(&mut self, ui: &mut Ui) -> Response {
        let (rect, response) = ui.allocate_exact_size(ui.available_size(), Sense::click_and_drag());

        ui.painter().rect_filled(rect, 0.0, Color32::BLACK);

        for canvas_photo in &mut self.state.photos {
            self.photo_manager.with_lock_mut(|photo_manager| {
                if let Ok(Some(texture)) = photo_manager.texture_for(&canvas_photo.photo, &ui.ctx())
                {
                    let mut transform_state = canvas_photo.transform_state.clone();
                    TransformableWidget::new(&mut transform_state).show(
                        ui,
                        |ui: &mut Ui, rect: Rect| {
                            Image::from_texture(texture)
                                
                                .rotate(
                                    canvas_photo.photo.metadata.rotation().radians(),
                                    Vec2::splat(0.5),
                                )
                                .fit_to_exact_size(rect.size())
                                .paint_at(ui, rect)
                        },
                    );

                    canvas_photo.transform_state = transform_state;
                }
            })
        }

        response
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
        add_contents: impl FnOnce(&mut Ui, Rect) -> R,
    ) -> Response {
        let response = ui.allocate_rect(self.state.rect, Sense::click_and_drag());

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

        for (handle, handle_pos) in &handles {
            let handle_rect = Rect::from_min_size(*handle_pos, handle_size);
            let response: Response = ui.interact(handle_rect, ui.id(), Sense::click_and_drag());

            if !response.is_pointer_button_down_on() && self.state.active_handle == Some(*handle) {
                self.state.active_handle = None;
            }

            if response.dragged()
                && (response
                    .interact_pointer_pos()
                    .and_then(|pos| Some(handle_rect.contains(pos)))
                    .unwrap_or(false)
                    || self.state.active_handle == Some(*handle))
            {
                let delta = response.drag_delta();

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
            if move_response.dragged() {
                let delta = move_response.drag_delta();
                self.state.rect = self.state.rect.translate(delta);
            }
        }

        let inner_response = add_contents(ui, rect);

        ui.painter()
            .rect_stroke(rect, 0.0, Stroke::new(3.0, Color32::RED));

        // Draw the resize handles
        for (_, handle_pos) in &handles {
            let handle_rect = Rect::from_min_size(*handle_pos, handle_size);
            ui.painter().rect_filled(handle_rect, 0.0, Color32::WHITE);
        }

        ui.ctx().pointer_latest_pos().map(|pos| {
            let mut cursor_icon = CursorIcon::Default;
            for (handle, handle_pos) in &handles {
                let handle_rect = Rect::from_min_size(*handle_pos, handle_size);
                if handle_rect.contains(pos) {
                    cursor_icon = handle.cursor();
                    break;
                } else if rect.contains(pos) {
                    cursor_icon = CursorIcon::Move;
                }
            }
            ui.ctx().set_cursor_icon(cursor_icon);
        });

        response
    }
}

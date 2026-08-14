use std::fmt;

use eframe::{
    egui::{self, Response, Sense, Widget},
    epaint::{Pos2, Rect, Vec2},
};
use egui::CursorIcon;

use crate::{
    cursor_manager::CursorManager,
    dep_mut,
    photo::Photo,
    photo_renderer::{PhotoRenderOptions, PhotoRenderStatus, PhotoRenderer},
    theme::color,
};

#[derive(Clone)]
pub struct ImageViewerState {
    pub scale: f32,
    pub offset: Vec2,
    focus_on_next_show: bool,
}

impl fmt::Debug for ImageViewerState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ImageViewerState")
            .field("scale", &self.scale)
            .field("offset", &self.offset)
            .field("focus_on_next_show", &self.focus_on_next_show)
            .finish()
    }
}

impl Default for ImageViewerState {
    fn default() -> Self {
        Self {
            scale: 1.0,
            offset: Vec2::ZERO,
            focus_on_next_show: true,
        }
    }
}

impl ImageViewerState {
    fn take_focus_request(&mut self) -> bool {
        std::mem::take(&mut self.focus_on_next_show)
    }
}

pub struct ImageViewer<'a> {
    photo: &'a Photo,
    state: &'a mut ImageViewerState,
}

impl<'a> ImageViewer<'a> {
    pub fn new(photo: &'a Photo, state: &'a mut ImageViewerState) -> Self {
        Self { photo, state }
    }

    pub fn show(self, ui: &mut eframe::egui::Ui) -> Response {
        self.ui(ui)
    }

    fn translate_from_center(offset: Vec2, rect: Rect, relative_to: Rect) -> Rect {
        let mut new_rect = rect;

        new_rect.set_center(Pos2::new(
            relative_to.center().x + offset.x,
            relative_to.center().y + offset.y,
        ));

        new_rect
    }
}

impl<'a> Widget for ImageViewer<'a> {
    fn ui(self, ui: &mut eframe::egui::Ui) -> Response {
        let available_size = ui.available_size();

        let (rect, response) = ui.allocate_exact_size(available_size, Sense::click_and_drag());

        ui.painter().rect_filled(rect, 0.0, color::BLACK);

        claim_viewer_focus(&response, self.state);

        let photo_size = Vec2::new(
            self.photo.metadata.rotated_width() as f32,
            self.photo.metadata.rotated_height() as f32,
        );
        let mut image_rect =
            Rect::from_center_size(rect.center(), aspect_fit_size(photo_size, available_size));

        image_rect = Self::translate_from_center(self.state.offset, image_rect, rect);

        let mouse_input = ui.input(|input| {
            if let Some(mouse_pos) = input.pointer.hover_pos() {
                for event in &input.events {
                    let egui::Event::MouseWheel { delta, unit, .. } = event else {
                        continue;
                    };
                    let wheel_points = match unit {
                        egui::MouseWheelUnit::Point => delta.y,
                        egui::MouseWheelUnit::Line => delta.y * 20.0,
                        egui::MouseWheelUnit::Page => delta.y * rect.height(),
                    };
                    if wheel_points != 0.0 {
                        return Some((wheel_points, mouse_pos));
                    }
                }
            }

            None
        });

        if let Some((scroll_delta, mouse_pos)) = mouse_input
            && rect.contains(mouse_pos)
        {
            let scale_delta = if scroll_delta > 0.0 { 1.1 } else { 0.9 };
            image_rect = apply_zoom_at_pointer(self.state, image_rect, mouse_pos, scale_delta);
        } else {
            image_rect = scale_rect_from_center(image_rect, self.state.scale);
        }

        image_rect = Self::translate_from_center(self.state.offset, image_rect, rect);

        let image_rect_size = image_rect.size();

        let can_drag = image_rect.width() > rect.width() || image_rect.height() > rect.height();

        if can_drag {
            dep_mut!(CursorManager, |cursor_manager| {
                ui.input(|input| {
                    if let Some(pointer_pos) = input.pointer.latest_pos()
                        && rect.contains(pointer_pos)
                    {
                        if input.pointer.primary_down() {
                            cursor_manager.set_cursor(CursorIcon::Grabbing);
                        } else {
                            cursor_manager.set_cursor(CursorIcon::Grab);
                        }
                    }
                });
            });
        }

        // Adjust image_rect so it always fills rect, or is centered in rect
        if image_rect.width() >= rect.width() {
            self.state.offset.x += response.drag_delta().x;
            image_rect = Self::translate_from_center(self.state.offset, image_rect, rect);

            if image_rect.right() < rect.right() {
                image_rect.set_right(rect.right());
                image_rect.set_left(rect.right() - image_rect_size.x);

                self.state.offset.x = image_rect.center().x - rect.center().x;
            } else if image_rect.left() > rect.left() {
                image_rect.set_left(rect.left());
                image_rect.set_right(rect.left() + image_rect_size.x);

                self.state.offset.x = image_rect.center().x - rect.center().x;
            }
        }

        if image_rect.height() >= rect.height() {
            self.state.offset.y += response.drag_delta().y;
            image_rect = Self::translate_from_center(self.state.offset, image_rect, rect);

            if image_rect.bottom() < rect.bottom() {
                image_rect.set_bottom(rect.bottom());
                image_rect.set_top(rect.bottom() - image_rect_size.y);

                self.state.offset.y = image_rect.center().y - rect.center().y;
            } else if image_rect.top() > rect.top() {
                image_rect.set_top(rect.top());
                image_rect.set_bottom(rect.top() + image_rect_size.y);

                self.state.offset.y = image_rect.center().y - rect.center().y;
            }
        }

        if image_rect.width() <= rect.width() {
            image_rect.set_center(Pos2::new(rect.center().x, image_rect.center().y));
            self.state.offset.x = 0.0;
        }

        if image_rect.height() <= rect.height() {
            image_rect.set_center(Pos2::new(image_rect.center().x, rect.center().y));
            self.state.offset.y = 0.0;
        }

        image_rect = Self::translate_from_center(self.state.offset, image_rect, rect);

        let adjustments = self.photo.adjustments();
        match PhotoRenderer::paint(
            ui,
            self.photo,
            &adjustments,
            image_rect,
            PhotoRenderOptions::default()
                .with_clip_rect(rect)
                .with_render_key("image-viewer"),
        ) {
            Ok(PhotoRenderStatus::Pending) => {
                ui.painter()
                    .rect_filled(image_rect, 0.0, color::SURFACE_MUTED);
            }
            Ok(_) => {}
            Err(error) => {
                ui.painter().text(
                    image_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    format!("Error: {}", error),
                    egui::FontId::default(),
                    color::ERROR,
                );
            }
        }

        response
    }
}

fn aspect_fit_size(image_size: Vec2, bounds: Vec2) -> Vec2 {
    if image_size.x <= 0.0 || image_size.y <= 0.0 || bounds.x <= 0.0 || bounds.y <= 0.0 {
        return Vec2::ZERO;
    }

    image_size * (bounds.x / image_size.x).min(bounds.y / image_size.y)
}

fn claim_viewer_focus(response: &Response, state: &mut ImageViewerState) {
    if response.clicked() || response.drag_started() || state.take_focus_request() {
        response.request_focus();
    }
}

fn scale_rect_from_center(rect: Rect, scale: f32) -> Rect {
    let scaled_width_diff = rect.width() * scale - rect.width();
    let scaled_height_diff = rect.height() * scale - rect.height();
    rect.expand2(Vec2::new(scaled_width_diff * 0.5, scaled_height_diff * 0.5))
}

fn apply_zoom_at_pointer(
    state: &mut ImageViewerState,
    image_rect: Rect,
    mouse_pos: Pos2,
    requested_scale_delta: f32,
) -> Rect {
    let previous_scale = state.scale;
    let new_scale = (previous_scale * requested_scale_delta).clamp(0.1, 20.0);
    let applied_scale_delta = new_scale / previous_scale;
    let rel_mouse_pos_before = image_rect.center() - mouse_pos;

    state.scale = new_scale;
    state.offset += rel_mouse_pos_before * applied_scale_delta - rel_mouse_pos_before;

    scale_rect_from_center(image_rect, new_scale)
}

#[cfg(test)]
mod fit_tests {
    use super::*;
    use egui_kittest::Harness;

    #[test]
    fn portrait_image_uses_the_available_height() {
        assert_eq!(
            aspect_fit_size(Vec2::new(2.0, 3.0), Vec2::new(500.0, 400.0)),
            Vec2::new(800.0 / 3.0, 400.0)
        );
    }

    #[test]
    fn fit_handles_empty_dimensions() {
        assert_eq!(
            aspect_fit_size(Vec2::ZERO, Vec2::new(500.0, 400.0)),
            Vec2::ZERO
        );
    }

    #[test]
    fn new_viewer_requests_focus_exactly_once() {
        let mut state = ImageViewerState::default();

        assert!(state.take_focus_request());
        assert!(!state.take_focus_request());
    }

    #[test]
    fn zoom_at_limits_does_not_move_an_off_center_image() {
        let image_rect = Rect::from_center_size(Pos2::new(250.0, 200.0), Vec2::new(300.0, 200.0));
        let mouse_pos = Pos2::new(140.0, 120.0);

        let mut max_state = ImageViewerState {
            scale: 20.0,
            offset: Vec2::new(12.0, -8.0),
            focus_on_next_show: false,
        };
        let max_offset = max_state.offset;
        apply_zoom_at_pointer(&mut max_state, image_rect, mouse_pos, 1.1);
        assert_eq!(max_state.offset, max_offset);

        let mut min_state = ImageViewerState {
            scale: 0.1,
            offset: Vec2::new(-9.0, 6.0),
            focus_on_next_show: false,
        };
        let min_offset = min_state.offset;
        apply_zoom_at_pointer(&mut min_state, image_rect, mouse_pos, 0.9);
        assert_eq!(min_state.offset, min_offset);
    }

    #[test]
    fn fresh_viewer_state_reclaims_focus_but_does_not_steal_it_afterward() {
        struct FocusState {
            viewer: ImageViewerState,
            tag: String,
            viewer_has_focus: bool,
            tag_has_focus: bool,
        }

        let tag_id = egui::Id::new("viewer-focus-tag-field");
        let mut harness = Harness::new_ui_state(
            move |ui, state: &mut FocusState| {
                let viewer_response = ui.allocate_response(Vec2::new(200.0, 100.0), Sense::click());
                claim_viewer_focus(&viewer_response, &mut state.viewer);
                let tag_response = ui.add(egui::TextEdit::singleline(&mut state.tag).id(tag_id));
                state.viewer_has_focus = viewer_response.has_focus();
                state.tag_has_focus = tag_response.has_focus();
            },
            FocusState {
                viewer: ImageViewerState::default(),
                tag: String::new(),
                viewer_has_focus: false,
                tag_has_focus: false,
            },
        );

        harness.run();
        assert!(harness.state().viewer_has_focus);

        harness
            .ctx
            .memory_mut(|memory| memory.request_focus(tag_id));
        harness.run();
        assert!(harness.state().tag_has_focus);

        harness.state_mut().viewer.focus_on_next_show = true;
        harness.run();
        assert!(harness.state().viewer_has_focus);
    }
}

use egui::{Align, Color32, Layout, Rect, Response, Sense, Stroke, Ui, Vec2};

use crate::{theme, utils::EguiUiExt};

#[derive(Debug, Clone, Default)]
pub struct StatusBarState {
    pub log_viewer_open: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StatusBarResponse {
    pub log_viewer_toggled: bool,
}

pub struct StatusBar<'a> {
    state: &'a mut StatusBarState,
}

impl<'a> StatusBar<'a> {
    pub fn new(state: &'a mut StatusBarState) -> Self {
        Self { state }
    }

    pub fn show(&mut self, ui: &mut Ui) -> StatusBarResponse {
        let bar_rect = Rect::from_min_size(
            ui.next_widget_position(),
            Vec2::new(ui.available_width(), Self::height()),
        );

        ui.painter()
            .rect_filled(bar_rect, 0.0, theme::color::SURFACE_EXTRA_DARK);

        let layout = Layout::left_to_right(Align::Center).with_cross_justify(true);
        let left_key = ui.id().with("status_left");
        let right_key = ui.id().with("status_right");

        let content_height = Self::height();
        let mut response = StatusBarResponse::default();

        ui.sized(
            left_key,
            |ui| Self::left_content(ui),
            |ui, left_size, add_contents| {
                let left_rect = Rect::from_min_size(
                    bar_rect.left_top(),
                    Vec2::new(left_size.x, content_height),
                );
                Self::render_child(ui, layout, left_rect, "status_left", add_contents);
                ui.interact(left_rect, ui.id().with("left"), Sense::hover());
            },
        );

        ui.sized(
            right_key,
            |ui| Self::right_content(ui),
            |ui, right_size, _add_contents| {
                let right_rect = Rect::from_min_max(
                    egui::pos2(bar_rect.right() - right_size.x, bar_rect.top()),
                    bar_rect.right_bottom(),
                );
                Self::render_child(ui, layout, right_rect, "status_right", |ui| {
                    self.right_content_with_state(ui, &mut response);
                });
                ui.interact(right_rect, ui.id().with("right"), Sense::hover());
            },
        );

        ui.advance_cursor_after_rect(bar_rect);

        response
    }

    fn render_child(
        ui: &mut Ui,
        layout: Layout,
        rect: Rect,
        id_salt: &'static str,
        add_contents: impl FnOnce(&mut Ui),
    ) {
        let mut child_ui = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(id_salt)
                .max_rect(rect)
                .layout(layout),
        );
        add_contents(&mut child_ui);
    }

    fn left_content(ui: &mut Ui) {
        ui.label("Status bar");
    }

    fn right_content(ui: &mut Ui) {
        Self::log_viewer_button(ui, false);
    }

    fn right_content_with_state(&mut self, ui: &mut Ui, response: &mut StatusBarResponse) {
        if Self::log_viewer_button(ui, self.state.log_viewer_open)
            .on_hover_text("Toggle log viewer")
            .clicked()
        {
            self.state.log_viewer_open = !self.state.log_viewer_open;
            response.log_viewer_toggled = true;
        }
    }

    pub fn height() -> f32 {
        30.0
    }

    pub fn log_viewer_button(ui: &mut Ui, is_open: bool) -> Response {
        let (rect, response) = ui.allocate_exact_size(Vec2::splat(24.0), Sense::click());

        let fill = if is_open {
            theme::color::ACCENT_MUTED
        } else if response.hovered() {
            theme::color::SURFACE_MUTED
        } else {
            theme::color::SURFACE_DARK
        };

        ui.painter().rect_filled(rect, 3.0, fill);

        let stroke = Stroke::new(
            1.5,
            if is_open {
                theme::color::ACCENT
            } else {
                theme::color::WHITE
            },
        );

        let left = rect.left() + 6.0;
        let right = rect.right() - 6.0;
        for y in [
            rect.center().y - 5.0,
            rect.center().y,
            rect.center().y + 5.0,
        ] {
            ui.painter()
                .line_segment([egui::pos2(left, y), egui::pos2(right, y)], stroke);
        }

        response
    }
}

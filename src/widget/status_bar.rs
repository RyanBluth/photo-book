use egui::{
    Align, FontId, Layout, Rect, Response, RichText, Sense, Spinner, Stroke, Ui, UiBuilder, Vec2,
};

use crate::{app_status::AppStatus, dep, theme, utils::EguiUiExt};

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
            .rect_filled(bar_rect, 0.0, theme::color::SURFACE_XX_DARK);

        let inner_content_rect = bar_rect.shrink2(Vec2::new(12.0, 4.0));

        let layout = Layout::left_to_right(Align::Center).with_cross_justify(true);

        let content_height = Self::height() - 8.0;
        let mut response = StatusBarResponse::default();

        ui.sized(
            ui.id().with("status_left"),
            |ui| Self::left_content(ui),
            |ui, left_size, add_contents| {
                let left_rect = Rect::from_min_size(
                    inner_content_rect.left_top(),
                    Vec2::new(left_size.x, content_height),
                );
                ui.scope_builder(
                    UiBuilder::new().layout(layout).max_rect(left_rect),
                    add_contents,
                );
            },
        );

        ui.sized(
            ui.id().with("status_right"),
            |ui| self.right_content(ui, &mut response),
            |ui, right_size, add_contents| {
                let right_rect = Rect::from_min_max(
                    egui::pos2(
                        inner_content_rect.right() - right_size.x,
                        inner_content_rect.top(),
                    ),
                    inner_content_rect.right_bottom(),
                );
                ui.scope_builder(
                    UiBuilder::new().layout(layout).max_rect(right_rect),
                    add_contents,
                );
            },
        );
        response
    }

    fn left_content(ui: &mut Ui) {
        dep!(AppStatus, |app_status| {
            for item in app_status.iter() {
                ui.add(Spinner::new().size(12.0));
                ui.label(RichText::new(item.description()).font(FontId::monospace(12.0)));
            }
        });
    }

    fn right_content(&mut self, ui: &mut Ui, response: &mut StatusBarResponse) {
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
        let (rect, response) = ui.allocate_exact_size(Vec2::new(24.0, 18.0), Sense::click());

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

use egui::{Align, FontId, Layout, Rect, RichText, Spinner, Ui, UiBuilder, Vec2, Widget};

use crate::{
    app_status::AppStatus,
    assets::Asset,
    dep, dep_mut,
    session::Session,
    theme::{self},
    utils::EguiUiExt,
    widget::icon_button::IconButton,
};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StatusBarResponse {
    pub log_viewer_toggled: bool,
    pub left_sidebar_toggled: bool,
    pub right_sidebar_toggled: bool,
}

pub struct StatusBar;

impl StatusBar {
    pub fn new() -> Self {
        Self
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
            Self::left_content,
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
        let preferences = dep!(Session, |session| session.project_preferences.clone());
        ui.horizontal(|ui| {
            if IconButton::new(Asset::logs())
                .active(preferences.log_viewer_open)
                .ui(ui)
                .clicked()
            {
                dep_mut!(Session, |session| {
                    session.project_preferences.log_viewer_open =
                        !session.project_preferences.log_viewer_open;
                });
                response.log_viewer_toggled = true;
            }

            if IconButton::new(Asset::sidebar_left())
                .active(preferences.left_sidebar_open)
                .ui(ui)
                .on_hover_text("Toggle left sidebar")
                .clicked()
            {
                dep_mut!(Session, |session| {
                    session.project_preferences.left_sidebar_open =
                        !session.project_preferences.left_sidebar_open;
                });
                response.left_sidebar_toggled = true;
            }

            if IconButton::new(Asset::sidebar_right())
                .active(preferences.right_sidebar_open)
                .ui(ui)
                .on_hover_text("Toggle right sidebar")
                .clicked()
            {
                dep_mut!(Session, |session| {
                    session.project_preferences.right_sidebar_open =
                        !session.project_preferences.right_sidebar_open;
                });
                response.right_sidebar_toggled = true;
            }
        });
    }

    pub fn height() -> f32 {
        30.0
    }
}

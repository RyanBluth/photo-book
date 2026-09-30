use egui::{FontId, Rect, RichText, Spinner, Ui, UiBuilder, Vec2, Widget};

use crate::{
    app_status::AppStatus,
    assets::Asset,
    dep, dep_mut,
    session::Session,
    theme::{self},
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

        let content_height = Self::height() - 8.0;
        let mut response = StatusBarResponse::default();
        ui.scope_builder(UiBuilder::new().max_rect(inner_content_rect), |ui| {
            egui::Sides::new()
                .height(content_height)
                .show(ui, Self::left_content, |ui| {
                    self.right_content(ui, &mut response)
                });
        });
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
        ui.horizontal_centered(|ui| {
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
        });
    }

    pub fn height() -> f32 {
        34.0
    }
}

use egui::{Layout, Pos2, Rect, Sense, Theme, Ui, Vec2};

use crate::theme;

pub struct StatusBarState {}

pub struct StatusBar<'a> {
    state: &'a mut StatusBarState,
}

impl<'a> StatusBar<'a> {
    pub fn new(state: &'a mut StatusBarState) -> Self {
        Self { state }
    }

    pub fn show(&mut self, ui: &mut Ui) {
        ui.allocate_ui_with_layout(
            Vec2::new(ui.available_width(), Self::height()),
            Layout::left_to_right(egui::Align::Min),
            |ui| {
                ui.painter().rect_filled(
                    Rect::from_min_size(
                        ui.next_widget_position(),
                        Vec2::new(ui.available_width(), Self::height()),
                    ),
                    0.0,
                    theme::color::SURFACE_EXTRA_DARK,
                );
                ui.label("status bar");
            },
        );
    }

    pub fn height() -> f32 {
        30.0
    }
}

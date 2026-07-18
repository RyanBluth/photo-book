use egui::{Image, ImageSource, Sense, Widget};

use crate::{
    cursor_manager::CursorManager,
    dep_mut,
    egui::{Color32, Vec2},
    theme,
};

pub struct IconButton {
    image_source: ImageSource<'static>,
    tint: Color32,
    tint_active: Color32,
    active: bool,
    size: Vec2,
}

impl IconButton {
    pub fn new(image_source: impl Into<ImageSource<'static>>) -> Self {
        IconButton {
            image_source: image_source.into(),
            tint: theme::color::ICON,
            tint_active: theme::color::ICON_ACTIVE,
            active: false,
            size: Vec2::new(24.0, 24.0),
        }
    }

    pub fn tint_active(mut self, color: Color32) -> Self {
        self.tint_active = color;
        self
    }

    pub fn active(mut self, active: bool) -> Self {
        self.active = active;
        self
    }

    pub fn tint(mut self, color: Color32) -> Self {
        self.tint = color;
        self
    }

    pub fn size(mut self, size: Vec2) -> Self {
        self.size = size;
        self
    }

    fn color(&self) -> Color32 {
        if self.active {
            self.tint_active
        } else {
            self.tint
        }
    }
}

impl Widget for IconButton {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let (rect, response) = ui.allocate_exact_size(self.size, Sense::click());

        if response.hovered() {
            dep_mut!(CursorManager, |cursor_manager| cursor_manager
                .set_cursor(egui::CursorIcon::PointingHand))
        }

        let opacity = if ui.input(|input| input.pointer.primary_down() && response.hovered()) {
            0.75
        } else {
            1.0
        };

        let color = self.color().linear_multiply(opacity);

        ui.put(
            rect,
            Image::new(self.image_source)
                .max_size(self.size)
                .tint(color),
        );

        response
    }
}

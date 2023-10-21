use eframe::{
    egui::{Align2, Response, Sense, Ui, Vec2, Widget},
    epaint::{Color32, Rect},
};

pub struct Spacer {
    size: Vec2,
}

impl Spacer {
    pub fn new(width: f32, height: f32) -> Self {
        Self {
            size: Vec2::new(width, height),
        }
    }
}

impl Widget for Spacer {
    fn ui(self, ui: &mut Ui) -> Response {
        let rect = Rect::from_min_size(ui.min_rect().min, self.size);
        let response = ui.allocate_rect(rect, Sense::hover());
        ui.painter().rect_filled(rect, 0.0, Color32::TRANSPARENT);
        response
    }
}

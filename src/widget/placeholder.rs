use eframe::egui::{Color32, Response, Sense, Ui, Vec2, Widget};

pub struct RectPlaceholder {
    size: Vec2,
    color: Color32,
}

impl RectPlaceholder {
    pub fn new(size: Vec2, color: Color32) -> Self {
        Self { size, color }
    }
}

impl Widget for RectPlaceholder {
    fn ui(self, ui: &mut Ui) -> Response {
        let (rect, response) = ui.allocate_exact_size(self.size, Sense::hover());
        ui.painter().rect_filled(rect, 0.0, self.color);
        response
    }
}

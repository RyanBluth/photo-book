use eframe::egui::{Color32, Rect, Sense, Shape, Ui, Vec2, Widget, Response};

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
        let rect = Rect::from_min_size(ui.min_rect().min, self.size);
        let response = ui.allocate_rect(rect, Sense::hover());
        ui.painter().rect_filled(rect, 0.0, self.color);
        response
    }
}

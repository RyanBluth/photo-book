use eframe::egui::{Color32, Response, Sense, Ui, Vec2, Widget};

pub struct RectPlaceholder {
    size: Vec2,
    color: Color32,
    corner_radius: f32,
}

impl RectPlaceholder {
    pub fn new(size: Vec2, color: Color32, corner_radius: f32) -> Self {
        Self { size, color, corner_radius }
    }
}

impl Widget for RectPlaceholder {
    fn ui(self, ui: &mut Ui) -> Response {
        let (rect, response) = ui.allocate_exact_size(self.size, Sense::hover());
        ui.painter().rect_filled(rect, self.corner_radius, self.color);
        response
    }
}

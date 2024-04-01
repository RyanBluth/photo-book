use eframe::{
    egui::{Color32, Rect, Response, Sense, Ui, Vec2, Widget},
    epaint::Pos2,
};

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
        let mut rect = Rect::from_min_size(Pos2::ZERO, self.size);
        rect.set_center(ui.next_widget_position());
        let response = ui.allocate_rect(rect, Sense::hover());
        ui.painter().rect_filled(rect, 0.0, self.color);
        response
    }
}

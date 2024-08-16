use egui::{
    Rect, Response, Sense, Ui, Vec2, Widget
};


pub struct SegmentControl<'a, T: PartialEq + Clone> {
    segments: &'a [(T, String)],
    selected: &'a mut T,
}

impl<'a, T: PartialEq + Clone> SegmentControl<'a, T> {
    pub fn new(segments: &'a [(T, String)], selected: &'a mut T) -> Self {
        Self { segments, selected }
    }
}

impl<'a, T: PartialEq + Clone> Widget for SegmentControl<'a, T> {
    fn ui(self, ui: &mut Ui) -> Response {
        let segment_count = self.segments.len();
        let spacing = ui.spacing().item_spacing.x;
        let total_spacing = spacing * (segment_count as f32 - 1.0);
        let available_width = ui.available_width() - total_spacing;
        let segment_width = available_width / segment_count as f32;

        let height = ui.spacing().interact_size.y;
        let size = Vec2::new(available_width + total_spacing, height);
        let (rect, mut response) = ui.allocate_exact_size(size, Sense::click());

        if ui.is_rect_visible(rect) {
            let visuals = ui.style().visuals.clone();
            let painter = ui.painter();

            // Draw the background
            painter.rect_filled(rect, 5.0, visuals.extreme_bg_color);

            for (idx, (value, label)) in self.segments.iter().enumerate() {
                let segment_rect = Rect::from_min_size(
                    rect.min + Vec2::new(idx as f32 * (segment_width + spacing), 0.0),
                    Vec2::new(segment_width, height),
                );

                let is_selected = value == self.selected;

                let text_color = if is_selected {
                    visuals.selection.stroke.color
                } else {
                    visuals.text_color()
                };

                if is_selected {
                    painter.rect_filled(segment_rect, 5.0, visuals.selection.bg_fill);
                }

                painter.text(
                    segment_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    label,
                    egui::FontId::default(),
                    text_color,
                );

                if response.clicked_by(egui::PointerButton::Primary) && segment_rect.contains(response.interact_pointer_pos().unwrap()) {
                    *self.selected = value.clone();
                    response.mark_changed();
                }
            }
        }

        response
    }
}

use std::{fmt::Debug, hash::Hash};

use egui::{Rect, Response, Sense, Ui, Vec2, Widget};

pub struct SegmentControl<'a, T: PartialEq + Clone + Hash + Debug> {
    segments: &'a [(T, String)],
    selected: &'a mut T,
}

impl<'a, T: PartialEq + Clone + Hash + Debug> SegmentControl<'a, T> {
    pub fn new(segments: &'a [(T, String)], selected: &'a mut T) -> Self {
        Self { segments, selected }
    }
}

impl<'a, T: PartialEq + Clone + Hash + Debug> Widget for SegmentControl<'a, T> {
    fn ui(self, ui: &mut Ui) -> Response {
        let segment_count = self.segments.len();
        if segment_count == 0 {
            return ui.allocate_response(Vec2::ZERO, Sense::hover());
        }
        let spacing = ui.spacing().item_spacing.x;
        let total_spacing = spacing * (segment_count as f32 - 1.0);
        let available_width = ui.available_width() - total_spacing;
        let segment_width = available_width / segment_count as f32;

        let height = ui.spacing().interact_size.y;
        let size = Vec2::new(available_width + total_spacing, height);
        let (rect, mut response) = ui.allocate_exact_size(size, Sense::hover());

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
                let mut segment_response =
                    ui.interact(segment_rect, response.id.with(value), Sense::click());

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

                if segment_response.clicked() && !is_selected {
                    *self.selected = value.clone();
                    segment_response.mark_changed();
                    response.mark_changed();
                }

                response = response.union(segment_response);
            }
        }

        response
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::Harness;

    #[test]
    fn empty_segments_are_safe() {
        let mut selected = 0;
        let mut harness = Harness::new_ui(|ui| {
            let response = ui.add(SegmentControl::new(&[], &mut selected));
            assert!(!response.changed());
            assert_eq!(response.rect.size(), Vec2::ZERO);
        });
        harness.run();
    }

    #[test]
    fn segments_change_only_on_a_real_transition() {
        struct State {
            selected: i32,
            rect: Rect,
            ever_changed: bool,
        }

        impl Default for State {
            fn default() -> Self {
                Self {
                    selected: 0,
                    rect: Rect::NOTHING,
                    ever_changed: false,
                }
            }
        }

        let segments = [(0, "First".to_owned()), (1, "Second".to_owned())];
        let mut harness = Harness::new_ui_state(
            move |ui, state: &mut State| {
                let response = ui.add(SegmentControl::new(&segments, &mut state.selected));
                state.rect = response.rect;
                state.ever_changed |= response.changed();
            },
            State::default(),
        );

        harness.run();
        let rect = harness.state().rect;
        let first = egui::pos2(rect.left() + rect.width() * 0.25, rect.center().y);
        let second = egui::pos2(rect.left() + rect.width() * 0.75, rect.center().y);

        harness.drag_at(first);
        harness.run();
        harness.drop_at(first);
        harness.run();
        assert_eq!(harness.state().selected, 0);
        assert!(!harness.state().ever_changed);

        harness.drag_at(second);
        harness.run();
        harness.drop_at(second);
        harness.run();
        assert_eq!(harness.state().selected, 1);
        assert!(harness.state().ever_changed);
    }
}

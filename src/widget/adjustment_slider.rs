use std::ops::RangeInclusive;

use egui::{Align2, FontId, Key, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2, pos2};

use crate::{cursor_manager::CursorManager, dep_mut, theme::color};

const ROW_HEIGHT: f32 = 22.0;
const CORNER_RADIUS: f32 = 4.0;
const INNER_X: f32 = 10.0;
const TEXT_SIZE: f32 = 12.0;
const STEP: f32 = 0.01;
const DISPLAY_DECIMAL_STEP: f32 = 0.01;

pub struct AdjustmentSlider<'a> {
    label: &'a str,
    value: &'a mut f32,
    range: RangeInclusive<f32>,
    default: f32,
}

impl<'a> AdjustmentSlider<'a> {
    pub fn new(label: &'a str, value: &'a mut f32) -> Self {
        Self {
            label,
            value,
            range: -1.0..=1.0,
            default: 0.0,
        }
    }

    pub fn range(mut self, range: RangeInclusive<f32>) -> Self {
        self.range = range;
        self
    }

    pub fn default(mut self, default: f32) -> Self {
        self.default = default;
        self
    }

    pub fn show(self, ui: &mut Ui) -> Response {
        let width = ui.available_width();
        let hit_height = ROW_HEIGHT.max(ui.spacing().interact_size.y);
        let (hit_rect, mut response) =
            ui.allocate_exact_size(Vec2::new(width, hit_height), Sense::click_and_drag());
        let rect =
            Rect::from_center_size(hit_rect.center(), Vec2::new(hit_rect.width(), ROW_HEIGHT));
        response
            .widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Slider, true, self.label));

        let previous_value = *self.value;

        if response.hovered() {
            dep_mut!(CursorManager, |cursor_manager| cursor_manager
                .set_cursor(egui::CursorIcon::ResizeHorizontal));
        }

        if response.double_clicked() {
            *self.value = self.default;
            response.mark_changed();
        } else if response.dragged() || response.clicked() {
            if let Some(pointer_pos) = response.interact_pointer_pos() {
                *self.value = value_from_x(pointer_pos.x, rect, &self.range);
                response.mark_changed();
            }
        }

        if response.has_focus() {
            ui.input(|input| {
                if input.key_pressed(Key::ArrowLeft) {
                    *self.value -= STEP;
                    response.mark_changed();
                } else if input.key_pressed(Key::ArrowRight) {
                    *self.value += STEP;
                    response.mark_changed();
                }
            });
        }

        *self.value = snap_to_default(
            self.value.clamp(*self.range.start(), *self.range.end()),
            self.default,
        );

        if (*self.value - previous_value).abs() > f32::EPSILON {
            response.mark_changed();
        }

        if ui.is_rect_visible(hit_rect) {
            paint(
                ui,
                rect,
                self.label,
                *self.value,
                self.default,
                &self.range,
                &response,
            );
        }

        response
    }
}

fn value_from_x(x: f32, rect: Rect, range: &RangeInclusive<f32>) -> f32 {
    let t = ((x - rect.left()) / rect.width()).clamp(0.0, 1.0);
    egui::lerp(*range.start()..=*range.end(), t)
}

fn value_to_x(value: f32, rect: Rect, range: &RangeInclusive<f32>) -> f32 {
    let span = *range.end() - *range.start();
    if span.abs() <= f32::EPSILON {
        return rect.center().x;
    }

    rect.left() + ((value - *range.start()) / span).clamp(0.0, 1.0) * rect.width()
}

fn snap_to_default(value: f32, default: f32) -> f32 {
    if (value - default).abs() < DISPLAY_DECIMAL_STEP * 0.5 {
        default
    } else {
        value
    }
}

fn paint(
    ui: &Ui,
    rect: Rect,
    label: &str,
    value: f32,
    default: f32,
    range: &RangeInclusive<f32>,
    response: &Response,
) {
    let painter = ui.painter();
    let fill = if response.dragged() {
        color::SURFACE_MUTED.linear_multiply(1.22)
    } else if response.hovered() {
        color::SURFACE_MUTED.linear_multiply(1.08)
    } else {
        color::SURFACE_MUTED
    };

    painter.rect(rect, CORNER_RADIUS, fill, Stroke::NONE, StrokeKind::Inside);

    let default_x = value_to_x(default, rect, range);
    painter.line_segment(
        [pos2(default_x, rect.top()), pos2(default_x, rect.bottom())],
        Stroke::new(2.0, color::SURFACE_STRONG.linear_multiply(0.68)),
    );

    if (value - default).abs() > f32::EPSILON {
        let value_x = value_to_x(value, rect, range);
        let left = default_x.min(value_x);
        let right = default_x.max(value_x);
        let active_rect = Rect::from_min_max(pos2(left, rect.top()), pos2(right, rect.bottom()));
        painter.rect_filled(
            active_rect,
            CORNER_RADIUS,
            color::SURFACE_STRONG.linear_multiply(0.28),
        );
    }

    let text_color = if response.dragged() {
        color::WHITE
    } else {
        color::CONTROL_TEXT
    };
    painter.text(
        pos2(rect.left() + INNER_X, rect.center().y),
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(TEXT_SIZE),
        text_color,
    );
    painter.text(
        pos2(rect.right() - INNER_X, rect.center().y),
        Align2::RIGHT_CENTER,
        format!("{value:.2}"),
        FontId::proportional(TEXT_SIZE),
        text_color,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snap_to_default_matches_display_precision() {
        assert_eq!(snap_to_default(0.004, 0.0), 0.0);
        assert_eq!(snap_to_default(-0.004, 0.0), 0.0);
        assert_eq!(snap_to_default(0.496, 0.5), 0.5);
    }

    #[test]
    fn snap_to_default_preserves_visible_values() {
        assert_eq!(snap_to_default(0.006, 0.0), 0.006);
        assert_eq!(snap_to_default(0.494, 0.5), 0.494);
    }
}

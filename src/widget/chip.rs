use egui::{
    Rect, Response, Sense, Stroke, StrokeKind, TextFormat, Ui, Vec2, Widget,
    text::{LayoutJob, TextWrapping},
};

use crate::cursor_manager::CursorManager;
use crate::dep_mut;
use crate::theme::color;

const CHIP_HEIGHT: f32 = 24.0;
const CHIP_RADIUS: u8 = 6;
const CHIP_FONT_SIZE: f32 = 13.0;
const CHIP_MAX_TEXT_WIDTH: f32 = 180.0;
const CHIP_MIN_TEXT_WIDTH: f32 = 48.0;
const CHIP_CLOSE_SIZE: f32 = 13.0;
const CHIP_CLOSE_GAP: f32 = 5.0;
const CHIP_CLOSE_HALF_CROSS: f32 = 4.0;
const CHIP_PADDING: Vec2 = Vec2::new(9.0, 4.0);

#[derive(Clone)]
pub struct Chip<'a> {
    text: &'a str,
    selected: bool,
    closable: bool,
}

#[derive(Clone)]
pub struct ChipResponse {
    #[allow(dead_code)]
    pub response: Response,
    pub clicked: bool,
    pub close_clicked: bool,
}

impl ChipResponse {
    pub fn clicked(&self) -> bool {
        self.clicked
    }

    pub fn close_clicked(&self) -> bool {
        self.close_clicked
    }
}

impl<'a> Chip<'a> {
    pub fn new(text: &'a str) -> Self {
        Self {
            text,
            selected: false,
            closable: false,
        }
    }

    pub fn selected(mut self, selected: bool) -> Self {
        self.selected = selected;
        self
    }

    pub fn closable(mut self, closable: bool) -> Self {
        self.closable = closable;
        self
    }
}

impl<'a> Widget for Chip<'a> {
    fn ui(self, ui: &mut Ui) -> Response {
        let visuals = ui.style().visuals.clone();
        let text_color = if self.selected {
            visuals.text_color()
        } else {
            visuals.weak_text_color()
        };
        let close_size = if self.closable { CHIP_CLOSE_SIZE } else { 0.0 };
        let close_gap = if self.closable { CHIP_CLOSE_GAP } else { 0.0 };
        let max_text_width = (ui.available_width() - CHIP_PADDING.x * 2.0 - close_gap - close_size)
            .clamp(CHIP_MIN_TEXT_WIDTH, CHIP_MAX_TEXT_WIDTH);
        let mut layout_job = LayoutJob::single_section(
            self.text.to_string(),
            TextFormat {
                font_id: egui::FontId::proportional(CHIP_FONT_SIZE),
                color: text_color,
                ..Default::default()
            },
        );
        layout_job.wrap = TextWrapping::truncate_at_width(max_text_width);
        let text_galley = ui.painter().layout_job(layout_job);

        let total_width = text_galley.size().x + close_size + close_gap + CHIP_PADDING.x * 2.0;
        let size = Vec2::new(total_width, CHIP_HEIGHT);
        let (rect, response) = ui.allocate_exact_size(size, Sense::click());
        let close_rect = close_rect(rect);
        let close_hovered = self.closable
            && response
                .hover_pos()
                .map(|pointer_pos| close_rect.contains(pointer_pos))
                .unwrap_or(false);

        if response.hovered() {
            dep_mut!(CursorManager, |cursor_manager| {
                cursor_manager.set_cursor(egui::CursorIcon::PointingHand);
            });
        }

        if ui.is_rect_visible(rect) {
            let painter = ui.painter();

            let bg_color = if self.selected {
                color::ACCENT_MUTED
            } else if response.hovered() {
                color::SURFACE_MUTED
            } else {
                color::SURFACE_DARK
            };

            let stroke_color = if self.selected {
                color::BLUE_SOFT
            } else if response.hovered() {
                color::SURFACE_STRONG
            } else {
                color::SURFACE_MUTED
            };

            painter.rect_filled(rect, CHIP_RADIUS, bg_color);
            painter.rect_stroke(
                rect,
                CHIP_RADIUS,
                Stroke::new(1.0, stroke_color),
                StrokeKind::Outside,
            );

            let text_pos = egui::pos2(
                rect.left() + CHIP_PADDING.x,
                rect.center().y - text_galley.size().y / 2.0,
            );
            painter.galley(text_pos, text_galley, text_color);

            if self.closable {
                if close_hovered {
                    painter.circle_filled(
                        close_rect.center(),
                        CHIP_CLOSE_SIZE / 2.0,
                        color::WHITE_OVERLAY,
                    );
                }

                let cross_color = if close_hovered {
                    visuals.text_color()
                } else {
                    visuals.weak_text_color()
                };
                let cross_center = close_rect.center();

                painter.line_segment(
                    [
                        egui::pos2(
                            cross_center.x - CHIP_CLOSE_HALF_CROSS,
                            cross_center.y - CHIP_CLOSE_HALF_CROSS,
                        ),
                        egui::pos2(
                            cross_center.x + CHIP_CLOSE_HALF_CROSS,
                            cross_center.y + CHIP_CLOSE_HALF_CROSS,
                        ),
                    ],
                    Stroke::new(1.5, cross_color),
                );
                painter.line_segment(
                    [
                        egui::pos2(
                            cross_center.x + CHIP_CLOSE_HALF_CROSS,
                            cross_center.y - CHIP_CLOSE_HALF_CROSS,
                        ),
                        egui::pos2(
                            cross_center.x - CHIP_CLOSE_HALF_CROSS,
                            cross_center.y + CHIP_CLOSE_HALF_CROSS,
                        ),
                    ],
                    Stroke::new(1.5, cross_color),
                );
            }
        }

        response
    }
}

fn close_rect(rect: Rect) -> Rect {
    Rect::from_center_size(
        egui::pos2(
            rect.right() - CHIP_PADDING.x - CHIP_CLOSE_SIZE / 2.0,
            rect.center().y,
        ),
        Vec2::splat(CHIP_CLOSE_SIZE),
    )
}

#[allow(dead_code)]
pub fn chip(ui: &mut Ui, text: &str) -> ChipResponse {
    let chip = Chip::new(text);
    let response = ui.add(chip);

    ChipResponse {
        clicked: response.clicked(),
        close_clicked: false,
        response,
    }
}

pub fn chip_selectable(ui: &mut Ui, text: &str, selected: bool) -> ChipResponse {
    let chip = Chip::new(text).selected(selected);
    let response = ui.add(chip);

    ChipResponse {
        clicked: response.clicked(),
        close_clicked: false,
        response,
    }
}

#[allow(dead_code)]
pub fn chip_closable(ui: &mut Ui, text: &str) -> ChipResponse {
    let chip = Chip::new(text).closable(true);
    let response = ui.add(chip);

    let close_clicked = if let Some(pointer_pos) = response.interact_pointer_pos() {
        if response.clicked() {
            close_rect(response.rect).contains(pointer_pos)
        } else {
            false
        }
    } else {
        false
    };

    ChipResponse {
        clicked: response.clicked() && !close_clicked,
        close_clicked,
        response,
    }
}

pub fn chip_selectable_closable(ui: &mut Ui, text: &str, selected: bool) -> ChipResponse {
    let chip = Chip::new(text).selected(selected).closable(true);
    let response = ui.add(chip);

    let close_clicked = if let Some(pointer_pos) = response.interact_pointer_pos() {
        if response.clicked() {
            close_rect(response.rect).contains(pointer_pos)
        } else {
            false
        }
    } else {
        false
    };

    ChipResponse {
        clicked: response.clicked() && !close_clicked,
        close_clicked,
        response,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use egui_kittest::Harness;
    #[cfg(all(feature = "wgpu", feature = "snapshot"))]
    use egui_kittest::SnapshotResults;

    #[test]
    fn test_basic_chip() {
        let mut harness = Harness::new_ui(|ui| {
            let response = chip(ui, "Test Chip");
            assert!(!response.clicked());
            assert!(!response.close_clicked());
        });

        harness.run();
    }

    #[test]
    fn test_chip_selectable() {
        let mut harness = Harness::new_ui(|ui| {
            let response = chip_selectable(ui, "Selected Chip", true);
            assert!(!response.clicked());
            assert!(!response.close_clicked());
        });

        harness.run();
    }

    #[test]
    fn test_chip_closable() {
        let mut harness = Harness::new_ui(|ui| {
            let response = chip_closable(ui, "Closable Chip");
            assert!(!response.clicked());
            assert!(!response.close_clicked());
        });

        harness.run();
    }

    #[test]
    fn test_chip_selectable_closable() {
        let mut harness = Harness::new_ui(|ui| {
            let response = chip_selectable_closable(ui, "Both Chip", false);
            assert!(!response.clicked());
            assert!(!response.close_clicked());
        });

        harness.run();
    }

    #[test]
    fn test_chip_widget_direct() {
        let mut harness = Harness::new_ui(|ui| {
            let chip = Chip::new("Direct Chip").selected(true).closable(true);
            let response = ui.add(chip);
            assert!(!response.clicked());
        });

        harness.run();
    }

    #[test]
    fn test_chip_text_rendering() {
        let test_texts = vec![
            "Short",
            "Medium Length Text",
            "Very Long Text That Should Still Render Properly",
            "🎨📸",
            "",
        ];

        for text in test_texts {
            let mut harness = Harness::new_ui(|ui| {
                let response = chip(ui, text);
                assert!(!response.clicked());
                assert!(!response.close_clicked());
            });

            harness.run();
        }
    }

    #[test]
    fn test_chip_response_methods() {
        let mut harness = Harness::new_ui(|ui| {
            let response = chip_selectable_closable(ui, "Test", true);

            // Test that methods exist and return expected types
            let _clicked: bool = response.clicked();
            let _close_clicked: bool = response.close_clicked();
            let _underlying_response: &Response = &response.response;
        });

        harness.run();
    }

    #[cfg(all(feature = "wgpu", feature = "snapshot"))]
    #[test]
    fn test_chip_visual_snapshots() {
        let mut results = SnapshotResults::new();

        // Basic chip
        let mut harness = Harness::new_ui(|ui| {
            chip(ui, "Basic");
        });
        harness.fit_contents();
        harness.snapshot("chip_basic");
        results.extend_harness(&mut harness);

        // Selected chip
        let mut harness = Harness::new_ui(|ui| {
            chip_selectable(ui, "Selected", true);
        });
        harness.fit_contents();
        harness.snapshot("chip_selected");
        results.extend_harness(&mut harness);

        // Unselected chip
        let mut harness = Harness::new_ui(|ui| {
            chip_selectable(ui, "Unselected", false);
        });
        harness.fit_contents();
        harness.snapshot("chip_unselected");
        results.extend_harness(&mut harness);

        // Closable chip
        let mut harness = Harness::new_ui(|ui| {
            chip_closable(ui, "Closable");
        });
        harness.fit_contents();
        harness.snapshot("chip_closable");
        results.extend_harness(&mut harness);

        // Selected and closable chip
        let mut harness = Harness::new_ui(|ui| {
            chip_selectable_closable(ui, "Both", true);
        });
        harness.fit_contents();
        harness.snapshot("chip_selected_closable");
        results.extend_harness(&mut harness);

        // Multiple chips with different states
        let mut harness = Harness::new_ui(|ui| {
            ui.horizontal(|ui| {
                chip(ui, "Basic");
                chip_selectable(ui, "Selected", true);
                chip_closable(ui, "Close");
                chip_selectable_closable(ui, "Both", false);
            });
        });
        harness.fit_contents();
        harness.snapshot("chip_multiple_states");
        results.extend_harness(&mut harness);

        // Test various text lengths
        let mut harness = Harness::new_ui(|ui| {
            ui.vertical(|ui| {
                chip(ui, "Short");
                chip(ui, "Medium Length Text");
                chip(ui, "Very Long Text That Should Display Properly");
                chip(ui, "🎨📸");
            });
        });
        harness.fit_contents();
        harness.snapshot("chip_text_variations");
        results.extend_harness(&mut harness);

        results.unwrap();
    }
}

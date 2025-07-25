use egui::{Color32, Rect, Response, Sense, Stroke, StrokeKind, Ui, Vec2, Widget};

#[derive(Clone)]
pub struct Chip<'a> {
    text: &'a str,
    selected: bool,
    closable: bool,
}

#[derive(Clone)]
pub struct ChipResponse {
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
        
        let text_galley = ui.painter().layout_no_wrap(
            self.text.to_string(),
            egui::FontId::default(),
            visuals.text_color(),
        );

        let close_button_size = if self.closable { 16.0 } else { 0.0 };
        let close_button_spacing = if self.closable { 4.0 } else { 0.0 };
        
        let padding = Vec2::new(12.0, 6.0);
        let total_width = text_galley.size().x + close_button_size + close_button_spacing + padding.x * 2.0;
        let height = text_galley.size().y.max(close_button_size) + padding.y * 2.0;
        
        let size = Vec2::new(total_width, height);
        let (rect, response) = ui.allocate_exact_size(size, Sense::click());

        if ui.is_rect_visible(rect) {
            let painter = ui.painter();
            
            let bg_color = if self.selected {
                visuals.selection.bg_fill
            } else if response.hovered() {
                visuals.widgets.hovered.bg_fill
            } else {
                visuals.widgets.inactive.bg_fill
            };

            let stroke_color = if self.selected {
                visuals.selection.stroke.color
            } else if response.hovered() {
                visuals.widgets.hovered.weak_bg_fill
            } else {
                visuals.widgets.inactive.weak_bg_fill
            };

            painter.rect_filled(rect, height / 2.0, bg_color);
            painter.rect_stroke(rect, height / 2.0, Stroke::new(1.0, stroke_color), StrokeKind::Outside);

            let text_color = if self.selected {
                visuals.selection.stroke.color
            } else {
                visuals.text_color()
            };

            let text_pos = egui::pos2(
                rect.left() + padding.x,
                rect.center().y - text_galley.size().y / 2.0,
            );
            painter.galley(text_pos, text_galley, text_color);

            if self.closable {
                let close_rect = Rect::from_center_size(
                    egui::pos2(
                        rect.right() - padding.x - close_button_size / 2.0,
                        rect.center().y,
                    ),
                    Vec2::splat(close_button_size),
                );

                let close_hovered = close_rect.contains(response.interact_pointer_pos().unwrap_or_default());
                let close_bg = if close_hovered {
                    Color32::from_rgba_unmultiplied(255, 255, 255, 50)
                } else {
                    Color32::TRANSPARENT
                };

                painter.circle_filled(close_rect.center(), close_button_size / 2.0, close_bg);
                
                let cross_color = if close_hovered {
                    Color32::WHITE
                } else {
                    text_color
                };

                let cross_size = 6.0;
                let cross_center = close_rect.center();
                let half_cross = cross_size / 2.0;

                painter.line_segment(
                    [
                        egui::pos2(cross_center.x - half_cross, cross_center.y - half_cross),
                        egui::pos2(cross_center.x + half_cross, cross_center.y + half_cross),
                    ],
                    Stroke::new(1.5, cross_color),
                );
                painter.line_segment(
                    [
                        egui::pos2(cross_center.x + half_cross, cross_center.y - half_cross),
                        egui::pos2(cross_center.x - half_cross, cross_center.y + half_cross),
                    ],
                    Stroke::new(1.5, cross_color),
                );
            }
        }

        response
    }
}

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

pub fn chip_closable(ui: &mut Ui, text: &str) -> ChipResponse {
    let chip = Chip::new(text).closable(true);
    let response = ui.add(chip);
    
    let close_clicked = if let Some(pointer_pos) = response.interact_pointer_pos() {
        if response.clicked() {
            let rect = response.rect;
            let padding = Vec2::new(12.0, 6.0);
            let close_button_size = 16.0;
            let close_rect = Rect::from_center_size(
                egui::pos2(
                    rect.right() - padding.x - close_button_size / 2.0,
                    rect.center().y,
                ),
                Vec2::splat(close_button_size),
            );
            close_rect.contains(pointer_pos)
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
            let rect = response.rect;
            let padding = Vec2::new(12.0, 6.0);
            let close_button_size = 16.0;
            let close_rect = Rect::from_center_size(
                egui::pos2(
                    rect.right() - padding.x - close_button_size / 2.0,
                    rect.center().y,
                ),
                Vec2::splat(close_button_size),
            );
            close_rect.contains(pointer_pos)
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
            let chip = Chip::new("Direct Chip")
                .selected(true)
                .closable(true);
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
        // Basic chip
        let mut harness = Harness::new_ui(|ui| {
            chip(ui, "Basic");
        });
        harness.fit_contents();
        harness.snapshot("chip_basic");

        // Selected chip
        let mut harness = Harness::new_ui(|ui| {
            chip_selectable(ui, "Selected", true);
        });
        harness.fit_contents();
        harness.snapshot("chip_selected");

        // Unselected chip
        let mut harness = Harness::new_ui(|ui| {
            chip_selectable(ui, "Unselected", false);
        });
        harness.fit_contents();
        harness.snapshot("chip_unselected");

        // Closable chip
        let mut harness = Harness::new_ui(|ui| {
            chip_closable(ui, "Closable");
        });
        harness.fit_contents();
        harness.snapshot("chip_closable");

        // Selected and closable chip
        let mut harness = Harness::new_ui(|ui| {
            chip_selectable_closable(ui, "Both", true);
        });
        harness.fit_contents();
        harness.snapshot("chip_selected_closable");

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
    }
}
use eframe::egui::{RichText, Ui};
use egui::{Color32, ComboBox, Stroke, StrokeKind};

use crate::utils::EditableValueTextEdit;

use super::layers::ShapeToolSettings;

pub struct ShapeToolControl<'a> {
    settings: &'a mut ShapeToolSettings,
    is_line_tool: bool,
}

impl<'a> ShapeToolControl<'a> {
    pub fn new(settings: &'a mut ShapeToolSettings) -> Self {
        Self {
            settings,
            is_line_tool: false,
        }
    }

    pub fn show(&mut self, ui: &mut Ui) {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing = egui::Vec2::new(10.0, 5.0);

            ui.label(RichText::new("Shape Tool Settings").heading());

            ui.horizontal(|ui| {
                ui.label("Color:");
                ui.color_edit_button_srgba(&mut self.settings.fill_color);
            });

            // Only show stroke kind for non-line shapes
            if !self.is_line_tool {
                // Add dropdown for stroke kind
                ui.horizontal(|ui| {
                    ui.label("Stroke Kind:");

                    let selected_label = match self.settings.stroke.map(|(_, kind)| kind) {
                        Some(StrokeKind::Inside) => "Inside",
                        Some(StrokeKind::Middle) => "Middle",
                        Some(StrokeKind::Outside) => "Outside",
                        None => "None",
                    };

                    let mut current_kind = self.settings.stroke.map(|(_, kind)| kind);

                    ComboBox::from_label("Stoke Kind")
                        .selected_text(selected_label)
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut current_kind, None, "None");
                            ui.selectable_value(
                                &mut current_kind,
                                Some(StrokeKind::Inside),
                                "Inside",
                            );
                            ui.selectable_value(
                                &mut current_kind,
                                Some(StrokeKind::Middle),
                                "Middle",
                            );
                            ui.selectable_value(
                                &mut current_kind,
                                Some(StrokeKind::Outside),
                                "Outside",
                            );
                        });

                    match current_kind {
                        Some(kind) => {
                            self.settings.stroke = Some((Stroke::new(1.0, Color32::BLACK), kind))
                        }
                        None => self.settings.stroke = None,
                    }
                });
            } else {
                // For lines, always show stroke controls (no "None" option)
                if self.settings.stroke.is_none() {
                    self.settings.stroke =
                        Some((Stroke::new(2.0, self.settings.fill_color), StrokeKind::Middle));
                }
            }

            if let Some((stroke_val, _)) = &mut self.settings.stroke {
                ui.horizontal(|ui| {
                    ui.label("Stroke Width:");
                    ui.text_edit_editable_value_singleline(
                        &mut self.settings.edit_state.stroke_width,
                    );

                    stroke_val.width = self.settings.edit_state.stroke_width.value();
                });

                ui.horizontal(|ui| {
                    ui.label("Stroke Color:");
                    ui.color_edit_button_srgba(&mut stroke_val.color);
                });
            }
        });
    }
}

use eframe::egui::Ui;
use egui::{ComboBox, Stroke, StrokeKind};

use crate::{theme::color, utils::EditableValueTextEdit};

use super::layers::ShapeToolSettings;
use super::property_control::{color_field, field, field_pair, property_group};

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
        let (stroke_width, stroke_color) = self
            .settings
            .stroke
            .map(|(stroke, _)| (stroke.width, stroke.color))
            .unwrap_or((1.0, color::BLACK));
        self.settings
            .edit_state
            .update(stroke_width, self.settings.fill_color, stroke_color);

        property_group(ui, |ui| {
            field(ui, "Fill color", |ui| {
                color_field(
                    ui,
                    &mut self.settings.fill_color,
                    &mut self.settings.edit_state.fill_color,
                );
            });

            // Only show stroke kind for non-line shapes
            if !self.is_line_tool {
                field(ui, "Stroke", |ui| {
                    let selected_label = match self.settings.stroke.map(|(_, kind)| kind) {
                        Some(StrokeKind::Inside) => "Inside",
                        Some(StrokeKind::Middle) => "Middle",
                        Some(StrokeKind::Outside) => "Outside",
                        None => "None",
                    };

                    let mut current_kind = self.settings.stroke.map(|(_, kind)| kind);

                    ComboBox::from_id_salt("shape_tool_stroke_alignment")
                        .selected_text(selected_label)
                        .width(ui.available_width())
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
                            if let Some((stroke, _)) = self.settings.stroke {
                                self.settings.stroke = Some((stroke, kind));
                            } else {
                                self.settings.stroke = Some((Stroke::new(1.0, color::BLACK), kind));
                            }
                        }
                        None => self.settings.stroke = None,
                    }
                });
            } else {
                // For lines, always show stroke controls (no "None" option)
                if self.settings.stroke.is_none() {
                    self.settings.stroke = Some((
                        Stroke::new(2.0, self.settings.fill_color),
                        StrokeKind::Middle,
                    ));
                }
            }

            if let Some((stroke_val, _)) = &mut self.settings.stroke {
                let (stroke_width, _) = field_pair(
                    ui,
                    |ui| {
                        field(ui, "Stroke width", |ui| {
                            ui.style_mut().spacing.text_edit_width = ui.available_width();
                            ui.text_edit_editable_value_singleline(
                                &mut self.settings.edit_state.stroke_width,
                            )
                        })
                    },
                    |ui| {
                        field(ui, "Stroke color", |ui| {
                            color_field(
                                ui,
                                &mut stroke_val.color,
                                &mut self.settings.edit_state.stroke_color,
                            );
                        });
                    },
                );
                stroke_val.width = stroke_width;
            }
        });
    }
}

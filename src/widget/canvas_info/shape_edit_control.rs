use eframe::egui::{self, RichText, Ui};
use egui::{Color32, ComboBox, Stroke, StrokeKind};

use crate::utils::EditableValueTextEdit;

use super::layers::{
    CanvasShapeKind, Layer,
    LayerContent::{Photo, Shape, TemplatePhoto, TemplateText, Text},
};

pub struct ShapeEditControl<'a> {
    layer: &'a mut Layer,
}

impl<'a> ShapeEditControl<'a> {
    pub fn new(layer: &'a mut Layer) -> Self {
        Self { layer }
    }

    pub fn show(&mut self, ui: &mut Ui) {
        let _response: egui::InnerResponse<()> = ui.allocate_ui(ui.available_size(), |ui| {
            match &mut self.layer.content {
                Photo(_) | TemplatePhoto { .. } | Text(_) | TemplateText { .. } => {
                    ui.label("No shape layer selected");
                }
                Shape(shape) => {
                    let is_line = matches!(shape.kind, CanvasShapeKind::Line);

                    ui.vertical(|ui| {
                        ui.spacing_mut().item_spacing = egui::Vec2::new(10.0, 5.0);

                        ui.label(RichText::new("Shape").heading());

                        ui.horizontal(|ui| {
                            ui.label("Shape Type:");
                            ui.label(format!("{:?}", shape.kind));
                        });

                        ui.horizontal(|ui| {
                            ui.label("Color:");
                            ui.color_edit_button_srgba(&mut shape.fill_color);
                        });

                        // Only show stroke kind for non-line shapes
                        if !is_line {
                            // Add dropdown for stroke kind
                            ui.horizontal(|ui| {
                                ui.label("Stroke Kind:");

                                let selected_label = match shape.stroke.map(|(_, kind)| kind) {
                                    Some(StrokeKind::Inside) => "Inside",
                                    Some(StrokeKind::Middle) => "Middle",
                                    Some(StrokeKind::Outside) => "Outside",
                                    None => "None",
                                };

                                let mut current_kind = shape.stroke.map(|(_, kind)| kind);

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
                                        shape.stroke = Some((Stroke::new(1.0, Color32::BLACK), kind))
                                    }
                                    None => shape.stroke = None,
                                }
                            });
                        } else {
                            // For lines, always show stroke controls (no "None" option)
                            if shape.stroke.is_none() {
                                shape.stroke =
                                    Some((Stroke::new(2.0, shape.fill_color), StrokeKind::Middle));
                            }
                        }

                        if let Some((stroke_val, _)) = &mut shape.stroke {
                            ui.horizontal(|ui| {
                                ui.label("Stroke Width:");
                                ui.text_edit_editable_value_singleline(
                                    &mut shape.edit_state.stroke_width,
                                );

                                stroke_val.width = shape.edit_state.stroke_width.value();
                            });

                            ui.horizontal(|ui| {
                                ui.label("Stroke Color:");
                                ui.color_edit_button_srgba(&mut stroke_val.color);
                            });
                        }
                    });
                }
            }
        });
    }
}
